#include "window_internal.h"

#include <windowsx.h>
#include <imm.h>
#include <limits.h> /* UINT_MAX, used by WHEEL_PAGESCROLL */
#include <math.h>
#include <string.h>
#include <wchar.h>

#define ZANT_TIMER_MODAL 4

/* ---------- events ---------- */

static int zant_modifiers(void) {
    int mods = 0;
    if (GetKeyState(VK_SHIFT) < 0) mods |= ZANT_MOD_SHIFT;
    if (GetKeyState(VK_CONTROL) < 0) mods |= ZANT_MOD_CONTROL;
    if (GetKeyState(VK_MENU) < 0) mods |= ZANT_MOD_ALT;
    if (GetKeyState(VK_LWIN) < 0 || GetKeyState(VK_RWIN) < 0) mods |= ZANT_MOD_META;
    if (GetKeyState(VK_CAPITAL) & 1) mods |= ZANT_MOD_CAPS_LOCK;
    return mods;
}

ZantEvent zant_event(ZantWindow *w, int type) {
    ZantEvent e;
    memset(&e, 0, sizeof(e));
    e.type = type;
    e.width = (int)lroundf(zant_logical_width(w));
    e.height = (int)lroundf(zant_logical_height(w));
    e.dpr = w->scale;
    return e;
}

static int zant_send(ZantWindow *w, ZantEvent *e) {
    if (!w || !w->host.event) return 0;
    const int handled = w->host.event(w->host.userdata, e);
    if (!handled) return handled;
    /* A click or a key press shows its result in the next frame now:
       WM_PAINT is the lowest-priority message and waits behind other input,
       which made fast clicking visibly lag. Pointer moves keep the coalescing
       invalidate (they arrive far faster than frames). */
    const bool discrete = e->type == ZANT_EVENT_POINTER_DOWN || e->type == ZANT_EVENT_POINTER_UP ||
                          e->type == ZANT_EVENT_KEY_DOWN || e->type == ZANT_EVENT_KEY_UP ||
                          e->type == ZANT_EVENT_TEXT_INPUT;
    if (discrete && w->hwnd && !w->closing && !w->in_draw) {
        RedrawWindow(w->hwnd, NULL, NULL, RDW_INVALIDATE | RDW_UPDATENOW);
    } else {
        zant_request(w);
    }
    return handled;
}

void zant_send_simple(ZantWindow *w, int type) {
    ZantEvent e = zant_event(w, type);
    zant_send(w, &e);
}

static void zant_send_pointer(ZantWindow *w, int type, int button, int x_px, int y_px) {
    ZantEvent e = zant_event(w, type);
    e.x = (float)x_px / w->scale;
    e.y = (float)y_px / w->scale;
    e.button = button;
    e.modifiers = zant_modifiers();
    zant_send(w, &e);
}

/* Zant key code: named keys, else the unshifted character, lowercased
   (what AppKit's charactersIgnoringModifiers gives for letters). */
static int zant_key_from_vk(WPARAM vk) {
    switch (vk) {
        case VK_LEFT: return ZANT_KEY_LEFT;
        case VK_RIGHT: return ZANT_KEY_RIGHT;
        case VK_UP: return ZANT_KEY_UP;
        case VK_DOWN: return ZANT_KEY_DOWN;
        case VK_HOME: return ZANT_KEY_HOME;
        case VK_END: return ZANT_KEY_END;
        case VK_PRIOR: return ZANT_KEY_PAGE_UP;
        case VK_NEXT: return ZANT_KEY_PAGE_DOWN;
        case VK_BACK: return ZANT_KEY_BACKSPACE;
        case VK_DELETE: return ZANT_KEY_DELETE;
        case VK_ESCAPE: return ZANT_KEY_ESCAPE;
        case VK_TAB: return ZANT_KEY_TAB;
        case VK_RETURN: return ZANT_KEY_ENTER;
        case VK_SPACE: return ' ';
        case VK_ADD: return '+';
        case VK_SUBTRACT: return '-';
        case VK_MULTIPLY: return '*';
        case VK_DIVIDE: return '/';
        case VK_DECIMAL: return '.';
        default: break;
    }
    if (vk >= VK_F1 && vk <= VK_F12) return ZANT_KEY_F1 + (int)(vk - VK_F1);
    if (vk >= 'A' && vk <= 'Z') return (int)(vk - 'A' + 'a');
    if (vk >= '0' && vk <= '9') return (int)vk;
    if (vk >= VK_NUMPAD0 && vk <= VK_NUMPAD9) return (int)('0' + (vk - VK_NUMPAD0));
    const UINT ch = MapVirtualKeyW((UINT)vk, MAPVK_VK_TO_CHAR) & 0x7FFF;
    if (ch >= 'A' && ch <= 'Z') return (int)(ch - 'A' + 'a');
    if (ch >= 0x20) return (int)ch;
    return 0;
}

/* ---------- window procedure ---------- */

static void zant_track_leave(ZantWindow *w, bool nonclient) {
    TRACKMOUSEEVENT tme;
    memset(&tme, 0, sizeof(tme));
    tme.cbSize = sizeof(tme);
    tme.dwFlags = TME_LEAVE | (nonclient ? TME_NONCLIENT : 0);
    tme.hwndTrack = w->hwnd;
    TrackMouseEvent(&tme);
    if (nonclient) w->tracking_nc = true;
    else w->tracking_mouse = true;
}

static void zant_pointer_move(ZantWindow *w, int x, int y) {
    if (!w->pointer_inside) {
        w->pointer_inside = true;
        zant_send_pointer(w, ZANT_EVENT_POINTER_ENTER, ZANT_POINTER_BUTTON_LEFT, x, y);
    }
    zant_send_pointer(w, ZANT_EVENT_POINTER_MOVE, ZANT_POINTER_BUTTON_LEFT, x, y);
}

static void zant_pointer_leave(ZantWindow *w) {
    if (!w->pointer_inside) return;
    w->pointer_inside = false;
    ZantEvent e = zant_event(w, ZANT_EVENT_POINTER_LEAVE);
    e.x = -1;
    e.y = -1;
    zant_send(w, &e);
}

static bool zant_close_vetoed(ZantWindow *w) {
    if (w->close_approved || w->closing) return false;
    ZantEvent e = zant_event(w, ZANT_EVENT_WINDOW_CLOSE_REQUEST);
    const int veto = w->host.event ? w->host.event(w->host.userdata, &e) : 0;
    zant_request(w);
    if (veto) return true;
    w->close_approved = true;
    return false;
}

LRESULT CALLBACK zant_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    ZantWindow *w = (ZantWindow *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (!w) return DefWindowProcW(hwnd, msg, wp, lp);

    switch (msg) {
        case ZANT_WM_COVER:
            zant_cover_now(w, (int)wp);
            return 0;

        case WM_NCCALCSIZE:
            /* panel: the whole window is client area */
            if (wp && w->style == ZANT_WINDOW_PANEL) return 0;
            if (wp && w->custom_frame) {
                /* keep the system's left/right/bottom resize borders, drop the
                   caption (Windows Terminal's approach) */
                NCCALCSIZE_PARAMS *params = (NCCALCSIZE_PARAMS *)lp;
                const LONG top = params->rgrc[0].top;
                const LRESULT result = DefWindowProcW(hwnd, msg, wp, lp);
                params->rgrc[0].top = top;
                if (IsZoomed(hwnd)) params->rgrc[0].top += zant_frame_y(w);
                return result;
            }
            break;

        case WM_NCHITTEST:
            if (w->style == ZANT_WINDOW_PANEL || w->style == ZANT_WINDOW_OVERLAY || w->fullscreen) return HTCLIENT;
            if (w->custom_frame || w->movable_by_background || w->style == ZANT_WINDOW_BORDERLESS) {
                LRESULT hit = DefWindowProcW(hwnd, msg, wp, lp);
                if (hit != HTCLIENT) return hit;
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                return zant_custom_hit(w, pt.x, pt.y);
            }
            break;

        /* the app-drawn maximize button keeps HTMAXBUTTON (Win11 snap
           layouts flyout) but is still a Zant button: forward the pointer */
        case WM_NCMOUSEMOVE:
            if (w->custom_frame) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                if (!w->tracking_nc) zant_track_leave(w, true);
                if (wp == HTMAXBUTTON || wp == HTCAPTION) zant_pointer_move(w, pt.x, pt.y);
                else zant_pointer_leave(w);
            }
            break;
        case WM_NCMOUSELEAVE:
            w->tracking_nc = false;
            if (!w->tracking_mouse) zant_pointer_leave(w);
            break;
        case WM_NCLBUTTONDOWN:
        case WM_NCLBUTTONDBLCLK:
            if (w->custom_frame && wp == HTMAXBUTTON) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                zant_send_pointer(w, ZANT_EVENT_POINTER_DOWN, ZANT_POINTER_BUTTON_LEFT, pt.x, pt.y);
                return 0;
            }
            break;
        case WM_NCLBUTTONUP:
            if (w->custom_frame && wp == HTMAXBUTTON) {
                POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
                ScreenToClient(hwnd, &pt);
                zant_send_pointer(w, ZANT_EVENT_POINTER_UP, ZANT_POINTER_BUTTON_LEFT, pt.x, pt.y);
                return 0;
            }
            break;

        case WM_ERASEBKGND:
            return 1;

        case WM_PAINT: {
            PAINTSTRUCT ps;
            HDC dc = BeginPaint(hwnd, &ps);
            zant_render(w, dc);
            EndPaint(hwnd, &ps);
            return 0;
        }

        case ZANT_WM_HIDDEN_FRAME:
            w->hidden_frame_pending = false;
            if (zant_backgrounded(w)) zant_render(w, NULL);
            return 0;

        case ZANT_WM_WAKE:
            InterlockedExchange(&w->wake_pending, 0);
            if (!w->closing && w->host.wake) w->host.wake(w->host.userdata);
            zant_request(w);
            return 0;

        case WM_TIMER:
            if (wp == ZANT_TIMER_REDRAW) {
                KillTimer(hwnd, ZANT_TIMER_REDRAW);
                zant_request(w);
            } else if (wp == ZANT_TIMER_SNAPSHOT) {
                KillTimer(hwnd, ZANT_TIMER_SNAPSHOT);
                zant_take_snapshot(w);
            } else if (wp == ZANT_TIMER_EXIT) {
                KillTimer(hwnd, ZANT_TIMER_EXIT);
                w->close_approved = true;
                DestroyWindow(hwnd);
            } else if (wp == ZANT_TIMER_MODAL) {
                /* live resize / move runs its own loop: keep animations going.
                   While the size keeps changing, every WM_SIZE frame advances
                   them: an extra animation frame would take the next resize
                   frame's vsync (half the sizes drawn, two vsyncs late). */
                const long long now = zant_now_us();
                if (w->animating && now - w->last_resize_us >= 50000 && now - w->last_frame_us >= 8000) {
                    InvalidateRect(hwnd, NULL, FALSE);
                    UpdateWindow(hwnd);
                }
            }
            return 0;

        case WM_ENTERSIZEMOVE:
            w->in_size_move = true;
            /* 10 ms: fires at every system timer tick (15.6 ms by default; a
               16 ms timer would wait for two of them) */
            SetTimer(hwnd, ZANT_TIMER_MODAL, 10, NULL);
            return 0;
        case WM_EXITSIZEMOVE:
            w->in_size_move = false;
            KillTimer(hwnd, ZANT_TIMER_MODAL);
            /* remembered as soon as the user moved it: a process ended by
               sign-out or shutdown never reaches WM_CLOSE */
            zant_save_placement(w);
            zant_request(w);
            return 0;

        case WM_SIZE: {
            /* minimized: the client area is empty, the layout keeps its size */
            if (wp == SIZE_MINIMIZED) return 0;
            const int previous_w = w->width_px, previous_h = w->height_px;
            zant_update_client_size(w);
            if (wp != SIZE_MINIMIZED && (previous_w != w->width_px || previous_h != w->height_px)) {
                w->last_resize_us = zant_now_us();
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
                /* draw now so live resize never shows stretched content */
                InvalidateRect(hwnd, NULL, FALSE);
                UpdateWindow(hwnd);
            } else if (wp == SIZE_MAXIMIZED || wp == SIZE_RESTORED) {
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
            }
            return 0;
        }

        case WM_DPICHANGED: {
            zant_set_dpi(w, HIWORD(wp));
            if (w->style == ZANT_WINDOW_OVERLAY) {
                /* keeps covering its monitor (zant_window_cover_screen) */
                zant_update_client_size(w);
                zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
                zant_request(w);
                return 0;
            }
            const RECT *suggested = (const RECT *)lp;
            SetWindowPos(hwnd, NULL, suggested->left, suggested->top, suggested->right - suggested->left,
                         suggested->bottom - suggested->top, SWP_NOZORDER | SWP_NOACTIVATE);
            zant_update_client_size(w);
            zant_send_simple(w, ZANT_EVENT_WINDOW_RESIZE);
            zant_request(w);
            return 0;
        }

        case WM_GETMINMAXINFO: {
            MINMAXINFO *info = (MINMAXINFO *)lp;
            if (w->fullscreen) return 0;
            int outer_w = 0, outer_h = 0;
            zant_outer_size(w, w->min_width, w->min_height, &outer_w, &outer_h);
            info->ptMinTrackSize.x = outer_w;
            info->ptMinTrackSize.y = outer_h;
            if (w->max_width > 0 || w->max_height > 0) {
                zant_outer_size(w, w->max_width > 0 ? w->max_width : 100000.0f,
                                w->max_height > 0 ? w->max_height : 100000.0f, &outer_w, &outer_h);
                if (w->max_width > 0) info->ptMaxTrackSize.x = outer_w;
                if (w->max_height > 0) info->ptMaxTrackSize.y = outer_h;
            }
            return 0;
        }

        case WM_ACTIVATE: {
            const bool active = LOWORD(wp) != WA_INACTIVE;
            if (active != w->active) {
                w->active = active;
                zant_send_simple(w, active ? ZANT_EVENT_WINDOW_FOCUS : ZANT_EVENT_WINDOW_BLUR);
                zant_request(w);
            }
            break;
        }

        case WM_SETCURSOR:
            if (LOWORD(lp) == HTCLIENT) {
                SetCursor(zant_cursor_handle(w->cursor));
                return TRUE;
            }
            break;

        case WM_MOUSEMOVE:
            if (!w->tracking_mouse) zant_track_leave(w, false);
            zant_pointer_move(w, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        case WM_MOUSELEAVE:
            w->tracking_mouse = false;
            zant_pointer_leave(w);
            return 0;

        case WM_LBUTTONDOWN:
        case WM_RBUTTONDOWN:
        case WM_MBUTTONDOWN: {
            const int button = msg == WM_LBUTTONDOWN ? ZANT_POINTER_BUTTON_LEFT
                               : msg == WM_RBUTTONDOWN ? ZANT_POINTER_BUTTON_RIGHT
                                                       : ZANT_POINTER_BUTTON_MIDDLE;
            SetFocus(hwnd);
            SetCapture(hwnd);
            zant_send_pointer(w, ZANT_EVENT_POINTER_DOWN, button, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        }
        case WM_LBUTTONUP:
        case WM_RBUTTONUP:
        case WM_MBUTTONUP: {
            const int button = msg == WM_LBUTTONUP ? ZANT_POINTER_BUTTON_LEFT
                               : msg == WM_RBUTTONUP ? ZANT_POINTER_BUTTON_RIGHT
                                                     : ZANT_POINTER_BUTTON_MIDDLE;
            if (!(wp & (MK_LBUTTON | MK_RBUTTON | MK_MBUTTON))) ReleaseCapture();
            zant_send_pointer(w, ZANT_EVENT_POINTER_UP, button, GET_X_LPARAM(lp), GET_Y_LPARAM(lp));
            return 0;
        }

        case WM_MOUSEWHEEL:
        case WM_MOUSEHWHEEL: {
            POINT pt = {GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
            ScreenToClient(hwnd, &pt);
            UINT lines = 3;
            SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, &lines, 0);
            if (lines == WHEEL_PAGESCROLL || lines == 0) lines = 3;
            /* one notch = `lines` lines of ~20pt; precision touchpads send
               fractions of a notch */
            const float notches = (float)GET_WHEEL_DELTA_WPARAM(wp) / (float)WHEEL_DELTA;
            const float distance = notches * (float)lines * 20.0f;
            ZantEvent e = zant_event(w, ZANT_EVENT_SCROLL);
            e.x = (float)pt.x / w->scale;
            e.y = (float)pt.y / w->scale;
            e.modifiers = zant_modifiers();
            if (msg == WM_MOUSEWHEEL) {
                if (e.modifiers & ZANT_MOD_SHIFT) e.scroll_x = distance;
                else e.scroll_y = distance;
            } else {
                e.scroll_x = -distance;
            }
            zant_send(w, &e);
            return 0;
        }

        case WM_KEYDOWN:
        case WM_SYSKEYDOWN: {
            w->suppress_char = false;
            if (wp == VK_PROCESSKEY) break; /* the IME is handling it */
            if (wp == VK_SHIFT || wp == VK_CONTROL || wp == VK_MENU || wp == VK_LWIN || wp == VK_RWIN ||
                wp == VK_CAPITAL) {
                break;
            }
            ZantEvent e = zant_event(w, ZANT_EVENT_KEY_DOWN);
            e.key = zant_key_from_vk(wp);
            e.modifiers = zant_modifiers();
            e.is_repeat = (lp & (1 << 30)) ? 1 : 0;
            if (e.key != 0 && zant_send(w, &e)) {
                w->suppress_char = true;
                return 0;
            }
            /* Ctrl+W closes the window like the AppKit menu's ⌘W */
            if (msg == WM_KEYDOWN && wp == 'W' && (e.modifiers & ZANT_MOD_CONTROL) &&
                !(e.modifiers & (ZANT_MOD_ALT | ZANT_MOD_SHIFT))) {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                w->suppress_char = true;
                return 0;
            }
            break;
        }
        case WM_KEYUP:
        case WM_SYSKEYUP: {
            ZantEvent e = zant_event(w, ZANT_EVENT_KEY_UP);
            e.key = zant_key_from_vk(wp);
            e.modifiers = zant_modifiers();
            if (e.key != 0 && w->host.event) w->host.event(w->host.userdata, &e);
            break;
        }
        case WM_CHAR: {
            if (w->suppress_char) return 0;
            const WCHAR c = (WCHAR)wp;
            if (c >= 0xD800 && c <= 0xDBFF) {
                w->high_surrogate = c;
                return 0;
            }
            WCHAR units[2];
            int count = 0;
            if (c >= 0xDC00 && c <= 0xDFFF) {
                if (!w->high_surrogate) return 0;
                units[count++] = w->high_surrogate;
                units[count++] = c;
                w->high_surrogate = 0;
            } else {
                if (c < 0x20 || c == 0x7F) return 0; /* Enter, Tab, Ctrl+X... are keys */
                units[count++] = c;
            }
            zant_send_text(w, ZANT_EVENT_TEXT_INPUT, units, count, 0, 0);
            return 0;
        }
        case WM_SYSCHAR:
            if (wp == ' ') break; /* Alt+Space: system menu */
            return 0;             /* no beep for unhandled Alt+key */
        case WM_SYSCOMMAND:
            /* a lone Alt / F10 would enter menu mode (there is no menu) */
            if ((wp & 0xFFF0) == SC_KEYMENU && lp == 0) return 0;
            break;

        case WM_IME_SETCONTEXT:
            lp &= ~ISC_SHOWUICOMPOSITIONWINDOW; /* we draw the marked text */
            return DefWindowProcW(hwnd, msg, wp, lp);
        case WM_IME_STARTCOMPOSITION:
            zant_ime_position(w);
            return 0;
        case WM_IME_COMPOSITION:
            zant_ime_composition(w, lp);
            return 0;
        case WM_IME_ENDCOMPOSITION:
            zant_send_text(w, ZANT_EVENT_COMPOSITION, L"", 0, 0, 0);
            return 0;

        case WM_CLOSE:
            if (zant_close_vetoed(w)) return 0;
            zant_save_placement(w);
            DestroyWindow(hwnd);
            return 0;

        case WM_SETTINGCHANGE:
            /* light / dark app mode changed in Settings */
            if (lp && wcscmp((const wchar_t *)lp, L"ImmersiveColorSet") == 0) {
                zant_send_simple(w, ZANT_EVENT_APPEARANCE_CHANGE);
                zant_request(w);
            }
            break;

        case WM_DESTROY:
            zant_save_placement(w);
            w->closing = true;
            KillTimer(hwnd, ZANT_TIMER_REDRAW);
            KillTimer(hwnd, ZANT_TIMER_MODAL);
            zant_send_simple(w, ZANT_EVENT_WINDOW_CLOSE);
            /* the main window ends the app; other windows just close */
            if (w == g_main) PostQuitMessage(0);
            return 0;

        default:
            break;
    }
    return DefWindowProcW(hwnd, msg, wp, lp);
}

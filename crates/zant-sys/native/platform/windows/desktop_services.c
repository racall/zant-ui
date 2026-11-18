#define COBJMACROS
#include "window_internal.h"

#include <shellapi.h>
#include <commctrl.h>
#include <initguid.h>
#include <shobjidl.h>
#include <shlobj.h>
#include <oleidl.h>
#include <objbase.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

/* ---------- strings ---------- */

wchar_t *zant_widen(const char *utf8) {
    if (!utf8) return NULL;
    const int n = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, NULL, 0);
    if (n <= 0) return NULL;
    wchar_t *out = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
    if (!out) return NULL;
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, out, n);
    return out;
}

/* malloc'ed UTF-8 copy of a UTF-16 string (length in units, -1 = NUL terminated) */
char *zant_narrow(const wchar_t *wide, int length, int *out_len) {
    if (!wide) return NULL;
    const int n = WideCharToMultiByte(CP_UTF8, 0, wide, length, NULL, 0, NULL, NULL);
    if (n < 0) return NULL;
    char *out = (char *)malloc((size_t)n + 1);
    if (!out) return NULL;
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, wide, length, out, n, NULL, NULL);
    out[n] = '\0';
    if (out_len) *out_len = (length < 0 && n > 0) ? n - 1 : n;
    return out;
}

/* ---------- dialogs ---------- */

typedef struct ZantFilters {
    COMDLG_FILTERSPEC specs[2];
    int count;
    wchar_t *pattern; /* "*.md;*.markdown" */
    wchar_t *first_ext;
} ZantFilters;

static void zant_filters_init(ZantFilters *f, const char *extensions) {
    memset(f, 0, sizeof(*f));
    if (!extensions || !extensions[0]) return;
    wchar_t *wide = zant_widen(extensions);
    if (!wide) return;
    const size_t n = wcslen(wide);
    f->pattern = (wchar_t *)calloc(n * 3 + 8, sizeof(wchar_t));
    f->first_ext = (wchar_t *)calloc(n + 1, sizeof(wchar_t));
    if (!f->pattern || !f->first_ext) {
        free(wide);
        return;
    }
    wchar_t *context = NULL;
    for (wchar_t *part = wcstok(wide, L",", &context); part; part = wcstok(NULL, L",", &context)) {
        while (*part == L' ' || *part == L'.') part++;
        size_t len = wcslen(part);
        while (len > 0 && part[len - 1] == L' ') part[--len] = 0;
        if (len == 0) continue;
        if (!f->first_ext[0]) wcscpy(f->first_ext, part);
        if (f->pattern[0]) wcscat(f->pattern, L";");
        wcscat(f->pattern, L"*.");
        wcscat(f->pattern, part);
    }
    free(wide);
    if (f->pattern[0]) {
        f->specs[0].pszName = f->pattern; /* the pattern doubles as the label */
        f->specs[0].pszSpec = f->pattern;
        f->specs[1].pszName = L"*.*";
        f->specs[1].pszSpec = L"*.*";
        f->count = 2;
    }
}

static void zant_filters_free(ZantFilters *f) {
    free(f->pattern);
    free(f->first_ext);
}

static char *zant_dialog_result(IFileDialog *dialog) {
    IShellItem *item = NULL;
    char *result = NULL;
    if (SUCCEEDED(IFileDialog_GetResult(dialog, &item)) && item) {
        PWSTR path = NULL;
        if (SUCCEEDED(IShellItem_GetDisplayName(item, SIGDN_FILESYSPATH, &path)) && path) {
            result = zant_narrow(path, -1, NULL);
            CoTaskMemFree(path);
        }
        IShellItem_Release(item);
    }
    return result;
}

static HWND zant_dialog_owner(void) {
    return g_main ? g_main->hwnd : NULL;
}

char *zant_open_file_dialog_ext(const char *extensions) {
    zant_process_setup();
    IFileOpenDialog *dialog = NULL;
    if (FAILED(CoCreateInstance(&CLSID_FileOpenDialog, NULL, CLSCTX_INPROC_SERVER, &IID_IFileOpenDialog,
                                (void **)&dialog))) {
        return NULL;
    }
    ZantFilters filters;
    zant_filters_init(&filters, extensions);
    if (filters.count) IFileOpenDialog_SetFileTypes(dialog, (UINT)filters.count, filters.specs);
    FILEOPENDIALOGOPTIONS options = 0;
    IFileOpenDialog_GetOptions(dialog, &options);
    IFileOpenDialog_SetOptions(dialog, options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST);
    char *result = NULL;
    if (SUCCEEDED(IFileOpenDialog_Show(dialog, zant_dialog_owner()))) result = zant_dialog_result((IFileDialog *)dialog);
    IFileOpenDialog_Release(dialog);
    zant_filters_free(&filters);
    return result;
}

char *zant_save_file_dialog_ext(const char *default_name, const char *extensions) {
    zant_process_setup();
    IFileSaveDialog *dialog = NULL;
    if (FAILED(CoCreateInstance(&CLSID_FileSaveDialog, NULL, CLSCTX_INPROC_SERVER, &IID_IFileSaveDialog,
                                (void **)&dialog))) {
        return NULL;
    }
    ZantFilters filters;
    zant_filters_init(&filters, extensions);
    if (filters.count) {
        IFileSaveDialog_SetFileTypes(dialog, (UINT)filters.count, filters.specs);
        IFileSaveDialog_SetDefaultExtension(dialog, filters.first_ext);
    }
    if (default_name && default_name[0]) {
        wchar_t *name = zant_widen(default_name);
        if (name) {
            IFileSaveDialog_SetFileName(dialog, name);
            free(name);
        }
    }
    FILEOPENDIALOGOPTIONS options = 0;
    IFileSaveDialog_GetOptions(dialog, &options);
    IFileSaveDialog_SetOptions(dialog, options | FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT);
    char *result = NULL;
    if (SUCCEEDED(IFileSaveDialog_Show(dialog, zant_dialog_owner()))) result = zant_dialog_result((IFileDialog *)dialog);
    IFileSaveDialog_Release(dialog);
    zant_filters_free(&filters);
    return result;
}

void zant_dialog_free(char *value) {
    free(value);
}

typedef HRESULT(WINAPI *TaskDialogIndirectFn)(const TASKDIALOGCONFIG *, int *, int *, BOOL *);

int zant_confirm_dialog(const char *title, const char *message, const char *first, const char *second,
                        const char *third) {
    zant_process_setup();
    wchar_t *w_title = zant_widen(title ? title : "");
    wchar_t *w_message = zant_widen(message ? message : "");
    wchar_t *labels[3] = {zant_widen(first), zant_widen(second), zant_widen(third)};
    wchar_t *caption = g_main ? g_main->title : L"";
    int answer = 2;

    /* comctl32 v6 (the framework's manifest) has TaskDialogIndirect */
    HMODULE comctl = LoadLibraryW(L"comctl32.dll");
    TaskDialogIndirectFn task_dialog = comctl ? (TaskDialogIndirectFn)(void *)GetProcAddress(comctl, "TaskDialogIndirect") : NULL;
    if (task_dialog) {
        TASKDIALOG_BUTTON buttons[3];
        UINT count = 0;
        for (int i = 0; i < 3; i++) {
            if (!labels[i]) continue;
            buttons[count].nButtonID = 100 + i;
            buttons[count].pszButtonText = labels[i];
            count++;
        }
        TASKDIALOGCONFIG config;
        memset(&config, 0, sizeof(config));
        config.cbSize = sizeof(config);
        config.hwndParent = zant_dialog_owner();
        config.dwFlags = TDF_POSITION_RELATIVE_TO_WINDOW | (labels[2] ? TDF_ALLOW_DIALOG_CANCELLATION : 0);
        config.pszWindowTitle = caption;
        config.pszMainInstruction = w_title;
        config.pszContent = w_message;
        config.pButtons = buttons;
        config.cButtons = count;
        config.nDefaultButton = 100;
        int pressed = 0;
        if (SUCCEEDED(task_dialog(&config, &pressed, NULL, NULL))) {
            if (pressed >= 100 && pressed <= 102) answer = pressed - 100;
            else answer = labels[2] ? 2 : (labels[1] ? 1 : 0); /* Esc / close: the last (cancel) button */
        }
    } else {
        size_t n = wcslen(w_title ? w_title : L"") + wcslen(w_message ? w_message : L"") + 4;
        wchar_t *text = (wchar_t *)calloc(n, sizeof(wchar_t));
        if (text) {
            _snwprintf(text, n - 1, L"%ls\n\n%ls", w_title ? w_title : L"", w_message ? w_message : L"");
            const UINT type = labels[2] ? MB_YESNOCANCEL : (labels[1] ? MB_OKCANCEL : MB_OK);
            const int r = MessageBoxW(zant_dialog_owner(), text, caption, type | MB_ICONWARNING);
            answer = (r == IDYES || r == IDOK) ? 0 : (r == IDNO ? 1 : (labels[2] ? 2 : 1));
            free(text);
        }
    }
    free(w_title);
    free(w_message);
    for (int i = 0; i < 3; i++) free(labels[i]);
    return answer;
}

int zant_open_url(const char *url) {
    if (!url || !url[0]) return 0;
    zant_process_setup();
    /* scheme-less links open as https, like on macOS */
    const char *colon = strchr(url, ':');
    const char *slash = strchr(url, '/');
    char *full = NULL;
    if (!colon || (slash && slash < colon)) {
        const size_t n = strlen(url) + 9;
        full = (char *)malloc(n);
        if (!full) return 0;
        snprintf(full, n, "https://%s", url);
    }
    wchar_t *wide = zant_widen(full ? full : url);
    free(full);
    if (!wide) return 0;
    const HINSTANCE r = ShellExecuteW(zant_dialog_owner(), L"open", wide, NULL, NULL, SW_SHOWNORMAL);
    free(wide);
    return (INT_PTR)r > 32 ? 1 : 0;
}

int zant_reveal_path(const char *path) {
    if (!path || !path[0]) return 0;
    zant_process_setup();
    wchar_t *wide = zant_widen(path);
    if (!wide) return 0;
    for (wchar_t *c = wide; *c; c++) {
        if (*c == L'/') *c = L'\\';
    }
    /* the shell's own "show in folder": opens (or reuses) an Explorer window
       with the item selected */
    int ok = 0;
    PIDLIST_ABSOLUTE item = ILCreateFromPathW(wide);
    if (item) {
        const HRESULT init = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        ok = SUCCEEDED(SHOpenFolderAndSelectItems(item, 0, NULL, 0)) ? 1 : 0;
        if (SUCCEEDED(init)) CoUninitialize();
        ILFree(item);
    }
    free(wide);
    return ok;
}

/* ---- launch at login: the user's Run key, the app started with --zant-login ---- */

#define ZANT_RUN_KEY L"Software\\Microsoft\\Windows\\CurrentVersion\\Run"
#define ZANT_LOGIN_ARG L"--zant-login"

int zant_app_launched_at_login(void) {
    static int known = -1;
    if (known < 0) {
        int argc = 0;
        wchar_t **argv = CommandLineToArgvW(GetCommandLineW(), &argc);
        known = 0;
        for (int i = 1; argv && i < argc; i++) {
            if (wcscmp(argv[i], ZANT_LOGIN_ARG) == 0) known = 1;
        }
        if (argv) LocalFree(argv);
    }
    return known;
}

/* What Settings > Apps > Startup and Task Manager's Startup tab record for a
   Run entry: 12 bytes, the first one odd when the user turned it off (the Run
   value stays, Windows skips it at logon). */
#define ZANT_STARTUP_APPROVED_KEY L"Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run"

static bool zant_startup_disabled_by_user(const wchar_t *name) {
    BYTE data[12] = {0};
    DWORD size = sizeof(data);
    if (RegGetValueW(HKEY_CURRENT_USER, ZANT_STARTUP_APPROVED_KEY, name, RRF_RT_REG_BINARY, NULL, data, &size) !=
            ERROR_SUCCESS ||
        size < 1)
        return false;
    return (data[0] & 1) != 0;
}

/* The executable a Run command starts ("path" args, or path args). */
static bool zant_run_command_is_self(const wchar_t *command) {
    wchar_t exe[MAX_PATH * 4];
    const DWORD n = GetModuleFileNameW(NULL, exe, (DWORD)(sizeof(exe) / sizeof(exe[0])));
    if (n == 0 || n >= sizeof(exe) / sizeof(exe[0])) return false;
    const wchar_t *start = command;
    while (*start == L' ') start++;
    const wchar_t *end;
    if (*start == L'"') {
        start++;
        end = wcschr(start, L'"');
    } else {
        end = wcschr(start, L' ');
    }
    const size_t length = end ? (size_t)(end - start) : wcslen(start);
    return length == wcslen(exe) &&
           CompareStringOrdinal(start, (int)length, exe, (int)length, TRUE) == CSTR_EQUAL;
}

int zant_launch_at_login_supported(void) {
    return 1;
}

int zant_launch_at_login_get(const char *app_id) {
    wchar_t *name = zant_widen(app_id);
    if (!name) return 0;
    wchar_t command[MAX_PATH * 4 + 64];
    DWORD size = sizeof(command);
    int state = 0;
    if (RegGetValueW(HKEY_CURRENT_USER, ZANT_RUN_KEY, name, RRF_RT_REG_SZ, NULL, command, &size) == ERROR_SUCCESS &&
        zant_run_command_is_self(command)) {
        /* an entry of another copy of the app (moved, an older install)
           does not start this one: off */
        state = zant_startup_disabled_by_user(name) ? 2 : 1;
    }
    free(name);
    return state;
}

int zant_launch_at_login_set(const char *app_id, int on) {
    wchar_t *name = zant_widen(app_id);
    if (!name) return 0;
    HKEY key = NULL;
    int ok = 0;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, ZANT_RUN_KEY, 0, NULL, 0, KEY_SET_VALUE, NULL, &key, NULL) == ERROR_SUCCESS) {
        if (on) {
            wchar_t exe[MAX_PATH * 4];
            const DWORD n = GetModuleFileNameW(NULL, exe, (DWORD)(sizeof(exe) / sizeof(exe[0])));
            wchar_t command[MAX_PATH * 4 + 64];
            if (n > 0 && n < sizeof(exe) / sizeof(exe[0])) {
                swprintf(command, sizeof(command) / sizeof(command[0]), L"\"%ls\" %ls", exe, ZANT_LOGIN_ARG);
                ok = RegSetValueExW(key, name, 0, REG_SZ, (const BYTE *)command,
                                    (DWORD)((wcslen(command) + 1) * sizeof(wchar_t))) == ERROR_SUCCESS;
            }
        } else {
            const LSTATUS rc = RegDeleteValueW(key, name);
            ok = rc == ERROR_SUCCESS || rc == ERROR_FILE_NOT_FOUND;
        }
        RegCloseKey(key);
    }
    /* The user turned the switch on in the app: an earlier "Disabled" from
       Task Manager would keep Windows from starting it (the usual way:
       mark it enabled). Off removes the record with the entry. */
    HKEY approved = NULL;
    if (ok && RegOpenKeyExW(HKEY_CURRENT_USER, ZANT_STARTUP_APPROVED_KEY, 0, KEY_SET_VALUE, &approved) ==
                  ERROR_SUCCESS) {
        if (!on) {
            RegDeleteValueW(approved, name);
        } else if (zant_startup_disabled_by_user(name)) {
            const BYTE enabled[12] = {0x02};
            ok = RegSetValueExW(approved, name, 0, REG_BINARY, enabled, sizeof(enabled)) == ERROR_SUCCESS;
        }
        RegCloseKey(approved);
    } else if (ok && on && zant_startup_disabled_by_user(name)) {
        /* still "Disabled" and that cannot be undone: Windows will not start it */
        ok = 0;
    }
    free(name);
    return ok;
}

void zant_open_login_item_settings(void) {
    ShellExecuteW(NULL, L"open", L"ms-settings:startupapps", NULL, NULL, SW_SHOWNORMAL);
}

size_t zant_device_name(char *out, size_t capacity) {
    wchar_t name[256];
    DWORD len = 256;
    if (!GetComputerNameExW(ComputerNamePhysicalDnsHostname, name, &len)) {
        len = 256;
        if (!GetComputerNameW(name, &len)) len = 0;
    }
    name[len] = 0;
    const int needed = WideCharToMultiByte(CP_UTF8, 0, name, -1, NULL, 0, NULL, NULL);
    if (out && capacity > 0) {
        if (WideCharToMultiByte(CP_UTF8, 0, name, -1, out, (int)capacity, NULL, NULL) == 0) out[capacity - 1] = 0;
    }
    return needed > 0 ? (size_t)needed : 1;
}

int zant_system_dark_mode(void) {
    DWORD light = 1, size = sizeof(light);
    if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                     L"AppsUseLightTheme", RRF_RT_REG_DWORD, NULL, &light, &size) != ERROR_SUCCESS)
        return 0;
    return light == 0 ? 1 : 0;
}

size_t zant_downloads_dir(char *out, size_t capacity) {
    /* FOLDERID_Downloads */
    static const GUID downloads = {0x374DE290, 0x123F, 0x4565, {0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B}};
    PWSTR wide = NULL;
    size_t result = 0;
    if (SUCCEEDED(SHGetKnownFolderPath(&downloads, 0, NULL, &wide)) && wide) {
        int len = 0;
        char *utf8 = zant_narrow(wide, -1, &len);
        if (utf8) {
            result = (size_t)len;
            if (out && capacity > 0) memcpy(out, utf8, (size_t)len < capacity ? (size_t)len : capacity);
            free(utf8);
        }
    }
    if (wide) CoTaskMemFree(wide);
    return result;
}

int zant_window_reduce_motion(void) {
    BOOL animations = TRUE;
    if (SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animations, 0)) return animations ? 0 : 1;
    return 0;
}

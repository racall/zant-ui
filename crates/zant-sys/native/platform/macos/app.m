#import "window_internal.h"
#import <Carbon/Carbon.h>
#import <CoreGraphics/CoreGraphics.h>
#import <dispatch/dispatch.h>

#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>

static id g_app_delegate = nil;

/* zant_app_quit: terminate without asking the main window again */
static bool g_quitting = false;
/* the event loop runs (applicationDidFinishLaunching came) */
bool g_launched = false;
/* the system opened the app at login (the launch Apple event says so) */
static bool g_launched_at_login = false;
/* a tray app: the Dock icon follows its normal windows (zant_update_dock) */
static bool g_dock_auto = false;

/* ---- app-level events (status bar, hotkeys, Dock) ---- */

static void (*g_app_handler)(void *, const ZantAppEvent *) = NULL;
static void *g_app_handler_userdata = NULL;

void zant_app_set_handler(void (*handler)(void *userdata, const ZantAppEvent *event), void *userdata) {
    g_app_handler = handler;
    g_app_handler_userdata = userdata;
}

void zant_app_emit(int type, int id) {
    if (!g_app_handler) return;
    ZantAppEvent event = {type, id};
    g_app_handler(g_app_handler_userdata, &event);
}

@interface ZantAppDelegate : NSObject <NSApplicationDelegate>
@end

@implementation ZantAppDelegate

/* The main window ends the app when it closes (windowWillClose); hidden
   windows and status bar apps keep running. */
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)sender {
    (void)sender;
    return NO;
}

/* [NSApp terminate:] exits the process after this: the app's last chance to
   stop its workers and save (ZANT_APP_EVENT_EXIT, like Windows after its loop). */
- (void)applicationWillTerminate:(NSNotification *)notification {
    (void)notification;
    zant_app_emit(ZANT_APP_EVENT_EXIT, 0);
}

- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    (void)sender;
    if (g_quitting) return NSTerminateNow;
    return zant_close_vetoed(g_main, 1) ? NSTerminateCancel : NSTerminateNow;
}

/* The launch Apple event tells whether the system opened the app at login
   (the way Chromium and others read it). */
static bool zant_launch_event_is_login(void) {
    NSAppleEventDescriptor *event = [[NSAppleEventManager sharedAppleEventManager] currentAppleEvent];
    if (!event || event.eventClass != kCoreEventClass || event.eventID != kAEOpenApplication) return false;
    return [[event paramDescriptorForKeyword:keyAEPropData] enumCodeValue] == keyAELaunchedAsLogInItem;
}

- (void)applicationWillFinishLaunching:(NSNotification *)notification {
    (void)notification;
    if (zant_launch_event_is_login()) g_launched_at_login = true;
}

/* Windows created with hidden_at_login appear now, unless opened at login. */
- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    (void)notification;
    if (zant_launch_event_is_login()) g_launched_at_login = true;
    g_launched = true;
    for (ZantWindow *w = g_windows; w; w = w->next) {
        if (!w->show_after_launch || w->closing) continue;
        w->show_after_launch = false;
        if (g_launched_at_login) continue;
        [w->window makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
    }
    zant_update_dock();
}

/* Dock icon clicked: the app decides (e.g. show its hidden main window);
   without a handler the main window is shown. */
- (BOOL)applicationShouldHandleReopen:(NSApplication *)sender hasVisibleWindows:(BOOL)visible {
    (void)sender;
    (void)visible;
    if (g_app_handler) {
        zant_app_emit(ZANT_APP_EVENT_REOPEN, 0);
    } else if (g_main && !g_main->closing) {
        zant_window_activate(g_main);
    }
    return YES;
}

@end

/* Chinese menu titles when the user's first language is Chinese, English
   otherwise (the template apps are Chinese; system items follow the OS). */
static NSString *zant_menu_text(NSString *en, NSString *zh) {
    NSString *language = [[NSLocale preferredLanguages] firstObject];
    return [language hasPrefix:@"zh"] ? zh : en;
}

static NSMenuItem *zant_menu_item(NSMenu *menu, NSString *title, SEL action, NSString *key,
                                  NSEventModifierFlags modifiers) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title action:action keyEquivalent:key];
    item.keyEquivalentModifierMask = modifiers;
    [menu addItem:item];
    return item;
}

static NSMenu *zant_submenu(NSMenu *main_menu, NSString *title) {
    NSMenuItem *item = [[NSMenuItem alloc] init];
    [main_menu addItem:item];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:title];
    item.submenu = menu;
    return menu;
}

/* The menu bar every Mac app has: app menu (About, Hide, Quit), File (Close),
   Edit (Undo … Select All, plus the system's dictation / emoji items), View
   (full screen), Window (Minimize, Zoom, the window list) and Help. */
static void zant_install_menu(const char *title) {
    NSString *name = [NSString stringWithUTF8String:title && title[0] ? title : "Zant UI"];
    const NSEventModifierFlags cmd = NSEventModifierFlagCommand;
    NSMenu *main_menu = [[NSMenu alloc] init];

    NSMenu *app_menu = zant_submenu(main_menu, name);
    zant_menu_item(app_menu, [zant_menu_text(@"About ", @"关于 ") stringByAppendingString:name],
                   @selector(orderFrontStandardAboutPanel:), @"", 0);
    [app_menu addItem:[NSMenuItem separatorItem]];
    NSMenu *services = [[NSMenu alloc] init];
    NSMenuItem *services_item = zant_menu_item(app_menu, zant_menu_text(@"Services", @"服务"), nil, @"", 0);
    services_item.submenu = services;
    NSApp.servicesMenu = services;
    [app_menu addItem:[NSMenuItem separatorItem]];
    zant_menu_item(app_menu, [zant_menu_text(@"Hide ", @"隐藏 ") stringByAppendingString:name],
                   @selector(hide:), @"h", cmd);
    zant_menu_item(app_menu, zant_menu_text(@"Hide Others", @"隐藏其他"), @selector(hideOtherApplications:),
                   @"h", cmd | NSEventModifierFlagOption);
    zant_menu_item(app_menu, zant_menu_text(@"Show All", @"全部显示"), @selector(unhideAllApplications:), @"", 0);
    [app_menu addItem:[NSMenuItem separatorItem]];
    zant_menu_item(app_menu, [zant_menu_text(@"Quit ", @"退出 ") stringByAppendingString:name],
                   @selector(terminate:), @"q", cmd);

    NSMenu *file_menu = zant_submenu(main_menu, zant_menu_text(@"File", @"文件"));
    zant_menu_item(file_menu, zant_menu_text(@"Close Window", @"关闭窗口"), @selector(performClose:), @"w", cmd);

    /* titled "Edit" in English too: AppKit adds Start Dictation / Emoji &
       Symbols to the menu with the Edit role */
    NSMenu *edit_menu = zant_submenu(main_menu, zant_menu_text(@"Edit", @"编辑"));
    zant_menu_item(edit_menu, zant_menu_text(@"Undo", @"撤销"), @selector(undo:), @"z", cmd);
    zant_menu_item(edit_menu, zant_menu_text(@"Redo", @"重做"), @selector(redo:), @"z", cmd | NSEventModifierFlagShift);
    [edit_menu addItem:[NSMenuItem separatorItem]];
    zant_menu_item(edit_menu, zant_menu_text(@"Cut", @"剪切"), @selector(cut:), @"x", cmd);
    zant_menu_item(edit_menu, zant_menu_text(@"Copy", @"拷贝"), @selector(copy:), @"c", cmd);
    zant_menu_item(edit_menu, zant_menu_text(@"Paste", @"粘贴"), @selector(paste:), @"v", cmd);
    zant_menu_item(edit_menu, zant_menu_text(@"Select All", @"全选"), @selector(selectAll:), @"a", cmd);

    NSMenu *view_menu = zant_submenu(main_menu, zant_menu_text(@"View", @"显示"));
    zant_menu_item(view_menu, zant_menu_text(@"Enter Full Screen", @"进入全屏幕"), @selector(toggleFullScreen:),
                   @"f", cmd | NSEventModifierFlagControl);

    NSMenu *window_menu = zant_submenu(main_menu, zant_menu_text(@"Window", @"窗口"));
    zant_menu_item(window_menu, zant_menu_text(@"Minimize", @"最小化"), @selector(performMiniaturize:), @"m", cmd);
    zant_menu_item(window_menu, zant_menu_text(@"Zoom", @"缩放"), @selector(performZoom:), @"", 0);
    [window_menu addItem:[NSMenuItem separatorItem]];
    zant_menu_item(window_menu, zant_menu_text(@"Bring All to Front", @"前置全部窗口"), @selector(arrangeInFront:), @"", 0);
    NSApp.windowsMenu = window_menu;

    NSMenu *help_menu = zant_submenu(main_menu, zant_menu_text(@"Help", @"帮助"));
    NSApp.helpMenu = help_menu;

    [NSApp setMainMenu:main_menu];
}

/* Unbundled runs (`zant run`, target/debug/app) have no AppIcon.icns: show
   the project's assets/icons/app-macos.png in the Dock instead, so the icon
   is visible while developing. Packaged apps use their bundle icon. */
static void zant_apply_dev_icon(void) {
    if ([[NSBundle mainBundle] objectForInfoDictionaryKey:@"CFBundleIconFile"]) return;
    NSString *exe = [[NSBundle mainBundle] executablePath];
    if (!exe) return;
    /* <project>/target/debug/app -> <project>/assets/icons/app-macos.png */
    NSString *project = [[[exe stringByDeletingLastPathComponent] stringByDeletingLastPathComponent]
        stringByDeletingLastPathComponent];
    NSString *path = [project stringByAppendingPathComponent:@"assets/icons/app-macos.png"];
    NSImage *icon = [[NSImage alloc] initWithContentsOfFile:path];
    if (icon) NSApp.applicationIconImage = icon;
}

void zant_app_init(const char *title) {
    static bool done = false;
    if (done) return;
    done = true;
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    zant_install_menu(title);
    zant_apply_dev_icon();
    g_app_delegate = [[ZantAppDelegate alloc] init];
    NSApp.delegate = g_app_delegate;
}

void zant_window_run(ZantWindow *window) {
    (void)window;
    @autoreleasepool {
        [NSApp run];
    }
}

/* ---- app ---- */

int zant_app_single_instance(const char *app_id) {
    @autoreleasepool {
        if (!app_id || !app_id[0]) return 1;
        NSString *ident = [NSString stringWithUTF8String:app_id];
        NSString *notice = [ident stringByAppendingString:@".activate"];
        /* an exclusive lock on a per-user file, held while the process lives */
        NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:[ident stringByAppendingString:@".lock"]];
        const int fd = open(path.fileSystemRepresentation, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
        if (fd >= 0 && flock(fd, LOCK_EX | LOCK_NB) != 0) {
            close(fd);
            [[NSDistributedNotificationCenter defaultCenter] postNotificationName:notice
                                                                           object:nil
                                                                         userInfo:nil
                                                               deliverImmediately:YES];
            return 0;
        }
        /* the first instance: later launches ask it to come forward */
        [[NSDistributedNotificationCenter defaultCenter] addObserverForName:notice
                                                                     object:nil
                                                                      queue:[NSOperationQueue mainQueue]
                                                                 usingBlock:^(NSNotification *note) {
            (void)note;
            if (g_app_handler) {
                zant_app_emit(ZANT_APP_EVENT_REOPEN, 0);
            } else if (g_main && !g_main->closing) {
                zant_window_activate(g_main);
            }
        }];
        return 1;
    }
}

void zant_app_quit(void) {
    g_quitting = true;
    [NSApp terminate:nil];
}

/* Tray apps (zant_tray_set): a Dock icon and menu bar only while a normal
   window is open or minimized, the standard switch between the regular and
   accessory activation policies (as menu bar apps do). */
void zant_update_dock(void) {
    if (!g_dock_auto) return;
    bool shown = false;
    for (ZantWindow *w = g_windows; w; w = w->next) {
        if (w->closing || zant_style_is_panel(w->style)) continue;
        if (w->window.isVisible || w->window.isMiniaturized) {
            shown = true;
            break;
        }
    }
    const NSApplicationActivationPolicy policy =
        shown ? NSApplicationActivationPolicyRegular : NSApplicationActivationPolicyAccessory;
    if (NSApp.activationPolicy == policy) return;
    [NSApp setActivationPolicy:policy];
    /* back to regular: AppKit leaves the menu bar with the previous app
       until the app activates again; activating on the next turn of the
       run loop brings the app's own menu bar */
    if (shown) {
        dispatch_async(dispatch_get_main_queue(), ^{ [NSApp activateIgnoringOtherApps:YES]; });
    }
}

int zant_app_launched_at_login(void) {
    return g_launched_at_login ? 1 : 0;
}

/* ---- status bar item (menu bar extra) ---- */

@interface ZantTrayTarget : NSObject
@end

@implementation ZantTrayTarget
- (void)itemSelected:(NSMenuItem *)item {
    zant_app_emit(ZANT_APP_EVENT_TRAY_MENU, (int)item.tag);
}
@end

static NSStatusItem *g_status_item = nil;
static ZantTrayTarget *g_tray_target = nil;

/* Apple HIG menu bar extras: an 18 pt template image (glyph ~15 pt) */
int zant_tray_icon_pixels(void) {
    return 36;
}

static void zant_tray_apply_icon(const uint8_t *rgba, int width, int height, float scale, int template_image,
                                 const char *tooltip) {
    if (rgba && width > 0 && height > 0) {
        NSData *data = [NSData dataWithBytes:rgba length:(size_t)width * (size_t)height * 4];
        CGDataProviderRef provider = CGDataProviderCreateWithCFData((__bridge CFDataRef)data);
        CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGImageRef cg = CGImageCreate((size_t)width, (size_t)height, 8, 32, (size_t)width * 4, space,
                                      kCGImageAlphaLast | kCGBitmapByteOrder32Big, provider, NULL,
                                      true, kCGRenderingIntentDefault);
        CGColorSpaceRelease(space);
        CGDataProviderRelease(provider);
        if (cg) {
            const CGFloat s = scale > 0 ? scale : 1;
            NSImage *image = [[NSImage alloc] initWithCGImage:cg size:NSMakeSize(width / s, height / s)];
            CGImageRelease(cg);
            image.template = template_image ? YES : NO;
            g_status_item.button.image = image;
        }
    }
    g_status_item.button.toolTip = tooltip ? [NSString stringWithUTF8String:tooltip] : nil;
}

int zant_tray_set(const uint8_t *rgba, const uint8_t *rgba_light, int width, int height, float scale,
                  int template_image, const char *tooltip, const ZantTrayItem *items, int count) {
    (void)rgba_light; /* the template image adapts to the menu bar */
    @autoreleasepool {
        zant_app_init(NULL);
        if (!g_status_item) {
            g_status_item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSSquareStatusItemLength];
            g_tray_target = [[ZantTrayTarget alloc] init];
        }
        zant_tray_apply_icon(rgba, width, height, scale, template_image, tooltip);
        g_dock_auto = true;
        zant_update_dock();
        NSMenu *menu = [[NSMenu alloc] init];
        menu.autoenablesItems = NO;
        for (int i = 0; i < count; i++) {
            const ZantTrayItem *it = &items[i];
            if (it->separator) {
                [menu addItem:[NSMenuItem separatorItem]];
                continue;
            }
            NSString *title_text = [NSString stringWithUTF8String:it->title ? it->title : ""];
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title_text
                                                          action:@selector(itemSelected:)
                                                   keyEquivalent:@""];
            item.target = g_tray_target;
            item.tag = it->id;
            item.state = it->checked ? NSControlStateValueOn : NSControlStateValueOff;
            item.enabled = it->disabled ? NO : YES;
            [menu addItem:item];
        }
        g_status_item.menu = count > 0 ? menu : nil;
        return 1;
    }
}

int zant_tray_set_icon(const uint8_t *rgba, const uint8_t *rgba_light, int width, int height, float scale,
                       int template_image, const char *tooltip) {
    (void)rgba_light;
    if (!g_status_item) return 0;
    @autoreleasepool {
        zant_tray_apply_icon(rgba, width, height, scale, template_image, tooltip);
    }
    return 1;
}

void zant_tray_remove(void) {
    if (!g_status_item) return;
    [[NSStatusBar systemStatusBar] removeStatusItem:g_status_item];
    g_status_item = nil;
}

/* ---- global hotkeys (Carbon RegisterEventHotKey: no accessibility
   permission needed) ---- */

enum { ZANT_MAX_HOTKEYS = 16 };
static struct {
    int id;
    EventHotKeyRef ref;
} g_hotkeys[ZANT_MAX_HOTKEYS];
static int g_hotkey_count = 0;
static EventHandlerRef g_hotkey_handler = NULL;

static OSStatus zant_hotkey_pressed(EventHandlerCallRef next, EventRef event, void *userdata) {
    (void)next;
    (void)userdata;
    EventHotKeyID hotkey;
    if (GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof(hotkey), NULL,
                          &hotkey) == noErr) {
        zant_app_emit(ZANT_APP_EVENT_HOTKEY, (int)hotkey.id);
    }
    return noErr;
}

/* Zant key -> macOS virtual key code (ANSI layout positions). */
static int zant_mac_keycode(int key) {
    static const int letters[26] = {0,  11, 8,  2,  14, 3,  5,  4,  34, 38, 40, 37, 46,
                                    45, 31, 35, 12, 15, 1,  17, 32, 9,  13, 7,  16, 6};
    static const int digits[10] = {29, 18, 19, 20, 21, 23, 22, 26, 28, 25};
    static const int fkeys[12] = {122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111};
    if (key >= 'a' && key <= 'z') return letters[key - 'a'];
    if (key >= 'A' && key <= 'Z') return letters[key - 'A'];
    if (key >= '0' && key <= '9') return digits[key - '0'];
    if (key >= ZANT_KEY_F1 && key <= ZANT_KEY_F12) return fkeys[key - ZANT_KEY_F1];
    switch (key) {
        case ZANT_KEY_SPACE: return 49;
        case ZANT_KEY_ENTER: return 36;
        case ZANT_KEY_TAB: return 48;
        case ZANT_KEY_ESCAPE: return 53;
        case ZANT_KEY_LEFT: return 123;
        case ZANT_KEY_RIGHT: return 124;
        case ZANT_KEY_DOWN: return 125;
        case ZANT_KEY_UP: return 126;
        case '`': return 50;
        case '-': return 27;
        case '=': return 24;
        case '[': return 33;
        case ']': return 30;
        case ';': return 41;
        case '\'': return 39;
        case ',': return 43;
        case '.': return 47;
        case '/': return 44;
        case '\\': return 42;
        default: return -1;
    }
}

void zant_hotkey_unregister(int id) {
    for (int i = 0; i < g_hotkey_count; i++) {
        if (g_hotkeys[i].id != id) continue;
        UnregisterEventHotKey(g_hotkeys[i].ref);
        g_hotkeys[i] = g_hotkeys[--g_hotkey_count];
        return;
    }
}

int zant_hotkey_register(int id, int key, int modifiers) {
    zant_hotkey_unregister(id);
    if (!(modifiers & (ZANT_MOD_CONTROL | ZANT_MOD_ALT | ZANT_MOD_META))) return 0;
    const int code = zant_mac_keycode(key);
    if (code < 0 || g_hotkey_count >= ZANT_MAX_HOTKEYS) return 0;
    if (!g_hotkey_handler) {
        EventTypeSpec spec = {kEventClassKeyboard, kEventHotKeyPressed};
        InstallApplicationEventHandler(&zant_hotkey_pressed, 1, &spec, NULL, &g_hotkey_handler);
    }
    UInt32 mods = 0;
    if (modifiers & ZANT_MOD_META) mods |= cmdKey;
    if (modifiers & ZANT_MOD_ALT) mods |= optionKey;
    if (modifiers & ZANT_MOD_CONTROL) mods |= controlKey;
    if (modifiers & ZANT_MOD_SHIFT) mods |= shiftKey;
    EventHotKeyID hotkey = {'ZANT', (UInt32)id};
    EventHotKeyRef ref = NULL;
    if (RegisterEventHotKey((UInt32)code, mods, hotkey, GetApplicationEventTarget(), 0, &ref) != noErr) return 0;
    g_hotkeys[g_hotkey_count].id = id;
    g_hotkeys[g_hotkey_count].ref = ref;
    g_hotkey_count++;
    return 1;
}

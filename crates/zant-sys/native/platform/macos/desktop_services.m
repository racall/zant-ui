#import "window.h"
#import <AppKit/AppKit.h>
#import <ServiceManagement/ServiceManagement.h>

#include <stdlib.h>
#include <string.h>

/* Only a packaged app (an .app bundle) can be a login item. */
static bool zant_is_app_bundle(void) {
    return [[[NSBundle mainBundle] bundlePath] hasSuffix:@".app"] && [NSBundle mainBundle].bundleIdentifier != nil;
}

int zant_launch_at_login_get(const char *app_id) {
    (void)app_id; /* the bundle's own identity */
    if (@available(macOS 13.0, *)) {
        if (!zant_is_app_bundle()) return 0;
        switch (SMAppService.mainAppService.status) {
            case SMAppServiceStatusEnabled: return 1;
            case SMAppServiceStatusRequiresApproval: return 2;
            default: return 0;
        }
    }
    return 0;
}

int zant_launch_at_login_supported(void) {
    if (@available(macOS 13.0, *)) return zant_is_app_bundle() ? 1 : 0;
    return 0;
}

int zant_launch_at_login_set(const char *app_id, int on) {
    (void)app_id;
    if (@available(macOS 13.0, *)) {
        if (!zant_is_app_bundle()) return -1;
        NSError *error = nil;
        SMAppService *service = SMAppService.mainAppService;
        const BOOL ok = on ? [service registerAndReturnError:&error] : [service unregisterAndReturnError:&error];
        if (!ok) NSLog(@"zant: login item: %@", error);
        /* The status is the answer, not the call: registering what is
           registered (kSMErrorAlreadyRegistered) or unregistering what is not
           (kSMErrorJobNotFound) is done already, and a registration the user
           has to allow in System Settings (requiresApproval, also after they
           turned it off there) is recorded; the app reads it with get. */
        const SMAppServiceStatus status = service.status;
        if (on) return (status == SMAppServiceStatusEnabled || status == SMAppServiceStatusRequiresApproval) ? 1 : 0;
        return (status == SMAppServiceStatusNotRegistered || status == SMAppServiceStatusNotFound) ? 1 : 0;
    }
    return -1;
}

void zant_open_login_item_settings(void) {
    if (@available(macOS 13.0, *)) {
        [SMAppService openSystemSettingsLoginItems];
        return;
    }
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preferences.users"]];
}

size_t zant_device_name(char *out, size_t capacity) {
    @autoreleasepool {
        NSString *name = [[NSHost currentHost] localizedName];
        if (name.length == 0) name = [[NSProcessInfo processInfo] hostName];
        const char *utf8 = name.UTF8String ? name.UTF8String : "";
        const size_t needed = strlen(utf8) + 1;
        if (out && capacity > 0) {
            const size_t n = needed < capacity ? needed : capacity;
            memcpy(out, utf8, n - 1);
            out[n - 1] = '\0';
        }
        return needed;
    }
}

int zant_system_dark_mode(void) {
    NSAppearance *appearance = NSApp ? NSApp.effectiveAppearance : [NSAppearance currentDrawingAppearance];
    NSAppearanceName name =
        [appearance bestMatchFromAppearancesWithNames:@[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]];
    return [name isEqualToString:NSAppearanceNameDarkAqua] ? 1 : 0;
}

size_t zant_downloads_dir(char *out, size_t capacity) {
    @autoreleasepool {
        NSArray<NSString *> *dirs = NSSearchPathForDirectoriesInDomains(NSDownloadsDirectory, NSUserDomainMask, YES);
        const char *utf8 = dirs.firstObject.UTF8String;
        const size_t len = utf8 ? strlen(utf8) : 0;
        if (out && capacity > 0) {
            const size_t n = len < capacity ? len : capacity;
            memcpy(out, utf8, n);
        }
        return len;
    }
}

int zant_window_reduce_motion(void) {
    return NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion ? 1 : 0;
}

void zant_dialog_free(char *value) {
    free(value);
}

static NSArray<NSString *> *zant_split_extensions(const char *extensions) {
    if (!extensions || extensions[0] == '\0') return nil;
    NSString *joined = [NSString stringWithUTF8String:extensions];
    NSMutableArray<NSString *> *result = [NSMutableArray array];
    for (NSString *part in [joined componentsSeparatedByString:@","]) {
        NSString *trimmed = [part stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]];
        if (trimmed.length > 0) [result addObject:trimmed];
    }
    return result.count > 0 ? result : nil;
}

/* extensions: comma separated list ("md,txt"); NULL or "" allows any type. */
char *zant_open_file_dialog_ext(const char *extensions) {
    @autoreleasepool {
        NSOpenPanel *panel = [NSOpenPanel openPanel];
        panel.canChooseFiles = YES;
        panel.canChooseDirectories = NO;
        panel.allowsMultipleSelection = NO;
        NSArray<NSString *> *types = zant_split_extensions(extensions);
        if (types) panel.allowedFileTypes = types;
        if ([panel runModal] != NSModalResponseOK) return NULL;
        NSURL *url = panel.URL;
        if (!url) return NULL;
        return strdup(url.path.fileSystemRepresentation);
    }
}

/* extensions: comma separated list ("csv,md"); NULL or "" allows any type. */
char *zant_save_file_dialog_ext(const char *default_name, const char *extensions) {
    @autoreleasepool {
        NSSavePanel *panel = [NSSavePanel savePanel];
        NSArray<NSString *> *types = zant_split_extensions(extensions);
        if (types) panel.allowedFileTypes = types;
        panel.allowsOtherFileTypes = YES;
        if (default_name) {
            panel.nameFieldStringValue = [NSString stringWithUTF8String:default_name];
        }
        if ([panel runModal] != NSModalResponseOK) return NULL;
        NSURL *url = panel.URL;
        if (!url) return NULL;
        return strdup(url.path.fileSystemRepresentation);
    }
}

/* Returns 0 for the first button, 1 for the second, 2 for the third. */
int zant_confirm_dialog(const char *title, const char *message, const char *first,
                        const char *second, const char *third) {
    @autoreleasepool {
        NSAlert *alert = [[NSAlert alloc] init];
        if (title) alert.messageText = [NSString stringWithUTF8String:title];
        if (message) alert.informativeText = [NSString stringWithUTF8String:message];
        if (first) [alert addButtonWithTitle:[NSString stringWithUTF8String:first]];
        if (second) [alert addButtonWithTitle:[NSString stringWithUTF8String:second]];
        if (third) [alert addButtonWithTitle:[NSString stringWithUTF8String:third]];
        const NSModalResponse response = [alert runModal];
        if (response == NSAlertFirstButtonReturn) return 0;
        if (response == NSAlertSecondButtonReturn) return 1;
        return 2;
    }
}

int zant_reveal_path(const char *path) {
    @autoreleasepool {
        if (!path || !path[0]) return 0;
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
        if (![[NSFileManager defaultManager] fileExistsAtPath:url.path]) return 0;
        [[NSWorkspace sharedWorkspace] activateFileViewerSelectingURLs:@[ url ]];
        return 1;
    }
}

int zant_open_url(const char *url) {
    @autoreleasepool {
        if (!url) return 0;
        NSString *string = [NSString stringWithUTF8String:url];
        NSURL *target = [NSURL URLWithString:string];
        if (!target || !target.scheme) {
            target = [NSURL URLWithString:[@"https://" stringByAppendingString:string]];
        }
        if (!target) return 0;
        return [[NSWorkspace sharedWorkspace] openURL:target] ? 1 : 0;
    }
}

// Handwritten runtime declarations included by zant-tsx-dts inside module "zant".
  export type u8 = number;
  export type u16 = number;
  export type u32 = number;
  export type u64 = number;
  export type usize = number;
  export type i8 = number;
  export type i16 = number;
  export type i32 = number;
  export type i64 = number;
  export type isize = number;
  export type f32 = number;
  export type f64 = number;

  export type Color = string | number;
  export type Edges = number | {
    top?: number; right?: number; bottom?: number; left?: number;
    horizontal?: number; vertical?: number;
  };
  export type Child = JSX.Element | boolean | null | undefined;
  export type Children = Child | readonly Children[];
  export type TextChild = string | number | boolean | null | undefined;
  export type TextChildren = TextChild | readonly TextChildren[];
  export type InlineChildren = TextChild | JSX.Element | readonly InlineChildren[];
  export interface Image { readonly __zantImage: unique symbol; }

  export type StateSetter<T> = (value: T | ((previous: T) => T)) => void;
  export function useState<T>(initial: T): [T, StateSetter<T>];
  export function useEffect(effect: () => void | (() => void), dependencies?: readonly unknown[]): void;
  export function useMemo<T>(compute: () => T, dependencies: readonly unknown[]): T;
  /** React's `useCallback`: the function itself (handlers are rebuilt every render). */
  export function useCallback<T extends (...args: never[]) => unknown>(callback: T, dependencies?: readonly unknown[]): T;
  export function useRef<T>(initial: T): { current: T };
  export function useTheme(): Theme;

  export interface Store<T> {
    get(): T;
    set(value: T): void;
    update(patch: Partial<T>): void;
  }
  export function createStore<T>(initial: T): Store<T>;
  export function useStore<T>(store: Store<T>): T;
  export function useStore<T, U>(store: Store<T>, selector: (state: T) => U): U;

  export interface KeyEvent {
    key: string;
    mod: boolean; shift: boolean; alt: boolean; ctrl: boolean; meta: boolean; repeat: boolean;
  }
  /** Root or a createWindow component: the close button / ⌘W; return false to keep the window. */
  export function useCloseRequest(callback: () => boolean): void;
  /** Root only: ⌘Q / Dock quit; return false to cancel. */
  export function useQuitRequest(callback: () => boolean): void;
  /** Root or a createWindow component: runs before the focused control; return true when handled. */
  export function useKeyDown(callback: (event: KeyEvent) => boolean): void;
  /** createWindow component only: the window gained or lost the keyboard. */
  export function useWindowFocus(callback: (focused: boolean) => void): void;
  /** Root only: the clipboard changed. */
  export function useClipboardChange(callback: () => void): void;
  /** Root only: the app was opened again (Dock icon). */
  export function useReopen(callback: () => void): void;
  /** Root only: a background thread called `ui::post_wake()`. */
  export function useWake(callback: () => void): void;
  /** Root only: the app is about to end (quit, main window closed); stop workers and save. */
  export function useAppExit(callback: () => void): void;

  export interface TrayItem {
    title?: string; checked?: boolean; disabled?: boolean; separator?: boolean;
    onSelect?: () => void;
  }
  export interface TrayOptions {
    icon: Uint8Array;
    iconLight?: Uint8Array;
    template?: boolean;
    tooltip?: string;
    items?: TrayItem[];
    onClick?: () => void;
  }
  export function useTray(options: TrayOptions): boolean;
  export function useHotkey(shortcut: string, callback: () => void): boolean;

  export interface WindowOptions {
    title?: string;
    style?: "native" | "hidden-titlebar" | "borderless" | "panel";
    width?: number; height?: number; minWidth?: number; minHeight?: number;
    background?: Color;
    resizable?: boolean; alwaysOnTop?: boolean; movableByBackground?: boolean; center?: boolean;
    titlebarHeight?: number;
  }
  export type WindowPlace = "center" | "topLeft" | "topRight" | "bottomLeft" | "bottomRight" | "trayCorner" | "pointer";
  export interface WindowHandle {
    show(): void;
    hide(): void;
    toggle(): void;
    activate(): void;
    visible(): boolean;
    /** Shown and has the keyboard. */
    active(): boolean;
    minimized(): boolean;
    place(where: WindowPlace, margin?: number): void;
    focus(key: string): void;
    /** The system title (title bar, Dock / taskbar), e.g. the open file's name. */
    setTitle(title: string): void;
    /** The content size in points; the top-left corner stays put. */
    setSize(width: number, height: number): void;
    setMinSize(width: number, height: number): void;
    /** 0 in a dimension leaves it unlimited. */
    setMaxSize(width: number, height: number): void;
    setAlwaysOnTop(on: boolean): void;
    /** macOS: its own Space; Windows: borderless over the monitor. */
    setFullscreen(on: boolean): void;
    fullscreen(): boolean;
  }
  export function createWindow(component: () => JSX.Element, options: WindowOptions): WindowHandle;

  // Desktop functions (crates/zant/src/tsx/platform.rs); the setters run in handlers and effects.
  export const clipboard: {
    text(): string;
    setText(text: string): boolean;
    /** PNG bytes, null when the clipboard holds no image. */
    image(): Uint8Array | null;
    setImage(png: Uint8Array): boolean;
    /** Copied files: absolute paths, one per line. */
    files(): string | null;
    setFiles(lines: string): boolean;
    changeCount(): number;
  };
  export const app: {
    quit(): void;
    /** "needsApproval": registered, but the user has to allow it in the system's settings
     * (macOS Login Items; Windows Task Manager / Settings > Apps > Startup turned it off). */
    launchAtLogin(): "off" | "on" | "needsApproval";
    /** false when the system refused; read launchAtLogin() afterwards (macOS may wait for approval). */
    setLaunchAtLogin(on: boolean): boolean;
    launchedAtLogin(): boolean;
    /** false for `zant run` builds (only packaged / release builds can start at login). */
    canLaunchAtLogin(): boolean;
    /** The system's list of apps that start at login (macOS Login Items, Windows Settings > Apps > Startup). */
    openLoginItemSettings(): void;
    deviceName(): string;
    downloadsDir(): string | null;
    dataDir(): string | null;
  };
  export const mainWindow: {
    show(): void;
    hide(): void;
    activate(): void;
    focus(key: string): void;
    visible(): boolean;
    active(): boolean;
    minimized(): boolean;
    /** The system title (title bar, Dock / taskbar), e.g. the open file's name. */
    setTitle(title: string): void;
    /** The content size in points; the top-left corner stays put. */
    setSize(width: number, height: number): void;
    setMinSize(width: number, height: number): void;
    /** 0 in a dimension leaves it unlimited. */
    setMaxSize(width: number, height: number): void;
    setAlwaysOnTop(on: boolean): void;
    /** macOS: its own Space; Windows: borderless over the monitor. */
    setFullscreen(on: boolean): void;
    fullscreen(): boolean;
  };
  /** Rebuild after `ms` milliseconds (call it while rendering). */
  export function wakeAfter(ms: number): void;
  /** The platform's label of a shortcut ("mod+f" → "⌘F" / "Ctrl+F"); a string literal. */
  export function keyLabel(spec: string): string;
  /** A shortcut string ("ctrl+cmd+v") as the platform shows it ("⌃⌘V"); "" when invalid. */
  export function shortcutLabel(shortcut: string): string;
  /** An imported image asset as an Image for <Image image>: decoded once, kept. */
  export function assetImage(png: Uint8Array): Image | null;
  export const isMacos: boolean;
  export const isWindows: boolean;
  export function titlebarInsets(): { left: number; right: number };

  // ── i18n (locales/*.json; the app's zant.i18n.d.ts fills these in) ──
  /** Message keys → their parameters (`undefined`: none). Generated per app. */
  export interface I18nMessages {}
  /** The app's locale tags. Generated per app. */
  export interface I18nLocales {}
  /** One of the app's locale tags (`"en"`, `"zh-CN"`). */
  export type Locale = keyof I18nLocales & string;
  type I18nKey = keyof I18nMessages & string;
  type I18nAll<U> = (U extends unknown ? (x: U) => void : never) extends (x: infer I) => void ? I : never;
  type I18nArgs<K extends I18nKey> = [I18nAll<I18nMessages[K]>] extends [undefined] ? [] : [params: I18nAll<I18nMessages[K]>];
  /** The message `key` in the current language, checked at compile time: `t("home.welcome", { name })`. */
  export function t<K extends I18nKey>(key: K, ...params: I18nArgs<K>): string;
  /** The current locale (re-read on every render). */
  export function useLocale(): Locale;
  /** Switch every window to `locale` (or back to the system language with "system"), remembered across launches; false for a locale the app lacks. Call it in a handler. */
  export function setLocale(locale: Locale | "system"): boolean;
  /** The user's choice: a locale or "system". */
  export function localePreference(): Locale | "system";
  /** The app's locales, sorted. */
  export function locales(): Locale[];
  /** A locale's own name ("简体中文", "English"), for language pickers. */
  export function localeName(locale: Locale): string;

  export interface ListProps<T> {
    key?: string | number;
    data: T[];
    renderItem: (item: T, index: number) => JSX.Element;
    keyExtractor?: (item: T) => string | number;
    estimatedItemHeight?: number; itemHeight?: number; followTail?: boolean; overdraw?: number;
    gap?: number; padding?: Edges; background?: Color;
    width?: number; height?: number; grow?: number;
    selectable?: boolean; focusable?: boolean; scrollbar?: boolean; alignment?: "top" | "bottom";
    /** Brings this item into view whenever it changes (e.g. the selected row). */
    scrollToIndex?: number;
    scrollAlign?: "start" | "center" | "end";
    /** A number that changes when the rows did (a new search, a delete): re-measures them and scrolls to scrollToIndex again. */
    layoutKey?: number;
  }
  export function List<T>(props: ListProps<T>): JSX.Element;

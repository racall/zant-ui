// A long settings page (5 sections, 40 switches with their own state):
// the heavy TSX case for `zant-tsx-frame-bench`. Written out in full because
// phase 1 has no `.map`; it reads like a hand-made settings screen.
import { useState, View, Row, Text, Switch } from "zant";

export default function Settings() {
  const [changes, setChanges] = useState<u32>(0);
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [showInMenuBar, setShowInMenuBar] = useState(false);
  const [checkForUpdates, setCheckForUpdates] = useState(false);
  const [sendUsageData, setSendUsageData] = useState(false);
  const [openLastProject, setOpenLastProject] = useState(false);
  const [restoreWindows, setRestoreWindows] = useState(false);
  const [confirmOnQuit, setConfirmOnQuit] = useState(false);
  const [showWelcomePage, setShowWelcomePage] = useState(false);
  const [lineNumbers, setLineNumbers] = useState(false);
  const [wordWrap, setWordWrap] = useState(false);
  const [autoSave, setAutoSave] = useState(false);
  const [formatOnSave, setFormatOnSave] = useState(false);
  const [showWhitespace, setShowWhitespace] = useState(false);
  const [highlightLine, setHighlightLine] = useState(false);
  const [bracketPairs, setBracketPairs] = useState(false);
  const [smoothCaret, setSmoothCaret] = useState(false);
  const [darkMode, setDarkMode] = useState(false);
  const [compactDensity, setCompactDensity] = useState(false);
  const [animations, setAnimations] = useState(false);
  const [largeIcons, setLargeIcons] = useState(false);
  const [statusBar, setStatusBar] = useState(false);
  const [tabBar, setTabBar] = useState(false);
  const [breadcrumbs, setBreadcrumbs] = useState(false);
  const [minimap, setMinimap] = useState(false);
  const [syncSettings, setSyncSettings] = useState(false);
  const [syncExtensions, setSyncExtensions] = useState(false);
  const [syncKeybindings, setSyncKeybindings] = useState(false);
  const [syncSnippets, setSyncSnippets] = useState(false);
  const [syncOnMetered, setSyncOnMetered] = useState(false);
  const [conflictPrompts, setConflictPrompts] = useState(false);
  const [backups, setBackups] = useState(false);
  const [syncHistory, setSyncHistory] = useState(false);
  const [crashReports, setCrashReports] = useState(false);
  const [telemetry, setTelemetry] = useState(false);
  const [spellCheckOnline, setSpellCheckOnline] = useState(false);
  const [linkPreviews, setLinkPreviews] = useState(false);
  const [remoteImages, setRemoteImages] = useState(false);
  const [locationHints, setLocationHints] = useState(false);
  const [readReceipts, setReadReceipts] = useState(false);
  const [typingStatus, setTypingStatus] = useState(false);

  return (
    <View padding={24} gap={20} overflow="scroll" height="100%">
      <Text key="changes" fontSize={20} fontWeight="semibold">Settings · {changes} changes</Text>
      <View key="general" gap={4}>
        <Text fontSize={16} fontWeight="medium">General</Text>
        <Row key="row-0" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Launch at login</Text>
            <Text fontSize={12}>{launchAtLogin ? "On" : "Off"} · option 1 of 40</Text>
          </View>
          <Switch key="sw-0" checked={launchAtLogin} onChange={(on) => { setLaunchAtLogin(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-1" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Show in menu bar</Text>
            <Text fontSize={12}>{showInMenuBar ? "On" : "Off"} · option 2 of 40</Text>
          </View>
          <Switch key="sw-1" checked={showInMenuBar} onChange={(on) => { setShowInMenuBar(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-2" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Check for updates</Text>
            <Text fontSize={12}>{checkForUpdates ? "On" : "Off"} · option 3 of 40</Text>
          </View>
          <Switch key="sw-2" checked={checkForUpdates} onChange={(on) => { setCheckForUpdates(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-3" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Send usage data</Text>
            <Text fontSize={12}>{sendUsageData ? "On" : "Off"} · option 4 of 40</Text>
          </View>
          <Switch key="sw-3" checked={sendUsageData} onChange={(on) => { setSendUsageData(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-4" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Open last project</Text>
            <Text fontSize={12}>{openLastProject ? "On" : "Off"} · option 5 of 40</Text>
          </View>
          <Switch key="sw-4" checked={openLastProject} onChange={(on) => { setOpenLastProject(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-5" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Restore windows</Text>
            <Text fontSize={12}>{restoreWindows ? "On" : "Off"} · option 6 of 40</Text>
          </View>
          <Switch key="sw-5" checked={restoreWindows} onChange={(on) => { setRestoreWindows(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-6" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Confirm on quit</Text>
            <Text fontSize={12}>{confirmOnQuit ? "On" : "Off"} · option 7 of 40</Text>
          </View>
          <Switch key="sw-6" checked={confirmOnQuit} onChange={(on) => { setConfirmOnQuit(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-7" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Show welcome page</Text>
            <Text fontSize={12}>{showWelcomePage ? "On" : "Off"} · option 8 of 40</Text>
          </View>
          <Switch key="sw-7" checked={showWelcomePage} onChange={(on) => { setShowWelcomePage(on); setChanges(changes + 1); }} />
        </Row>
      </View>
      <View key="editor" gap={4}>
        <Text fontSize={16} fontWeight="medium">Editor</Text>
        <Row key="row-8" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Line numbers</Text>
            <Text fontSize={12}>{lineNumbers ? "On" : "Off"} · option 9 of 40</Text>
          </View>
          <Switch key="sw-8" checked={lineNumbers} onChange={(on) => { setLineNumbers(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-9" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Word wrap</Text>
            <Text fontSize={12}>{wordWrap ? "On" : "Off"} · option 10 of 40</Text>
          </View>
          <Switch key="sw-9" checked={wordWrap} onChange={(on) => { setWordWrap(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-10" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Auto save</Text>
            <Text fontSize={12}>{autoSave ? "On" : "Off"} · option 11 of 40</Text>
          </View>
          <Switch key="sw-10" checked={autoSave} onChange={(on) => { setAutoSave(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-11" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Format on save</Text>
            <Text fontSize={12}>{formatOnSave ? "On" : "Off"} · option 12 of 40</Text>
          </View>
          <Switch key="sw-11" checked={formatOnSave} onChange={(on) => { setFormatOnSave(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-12" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Show whitespace</Text>
            <Text fontSize={12}>{showWhitespace ? "On" : "Off"} · option 13 of 40</Text>
          </View>
          <Switch key="sw-12" checked={showWhitespace} onChange={(on) => { setShowWhitespace(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-13" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Highlight line</Text>
            <Text fontSize={12}>{highlightLine ? "On" : "Off"} · option 14 of 40</Text>
          </View>
          <Switch key="sw-13" checked={highlightLine} onChange={(on) => { setHighlightLine(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-14" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Bracket pairs</Text>
            <Text fontSize={12}>{bracketPairs ? "On" : "Off"} · option 15 of 40</Text>
          </View>
          <Switch key="sw-14" checked={bracketPairs} onChange={(on) => { setBracketPairs(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-15" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Smooth caret</Text>
            <Text fontSize={12}>{smoothCaret ? "On" : "Off"} · option 16 of 40</Text>
          </View>
          <Switch key="sw-15" checked={smoothCaret} onChange={(on) => { setSmoothCaret(on); setChanges(changes + 1); }} />
        </Row>
      </View>
      <View key="appearance" gap={4}>
        <Text fontSize={16} fontWeight="medium">Appearance</Text>
        <Row key="row-16" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Dark mode</Text>
            <Text fontSize={12}>{darkMode ? "On" : "Off"} · option 17 of 40</Text>
          </View>
          <Switch key="sw-16" checked={darkMode} onChange={(on) => { setDarkMode(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-17" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Compact density</Text>
            <Text fontSize={12}>{compactDensity ? "On" : "Off"} · option 18 of 40</Text>
          </View>
          <Switch key="sw-17" checked={compactDensity} onChange={(on) => { setCompactDensity(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-18" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Animations</Text>
            <Text fontSize={12}>{animations ? "On" : "Off"} · option 19 of 40</Text>
          </View>
          <Switch key="sw-18" checked={animations} onChange={(on) => { setAnimations(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-19" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Large icons</Text>
            <Text fontSize={12}>{largeIcons ? "On" : "Off"} · option 20 of 40</Text>
          </View>
          <Switch key="sw-19" checked={largeIcons} onChange={(on) => { setLargeIcons(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-20" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Status bar</Text>
            <Text fontSize={12}>{statusBar ? "On" : "Off"} · option 21 of 40</Text>
          </View>
          <Switch key="sw-20" checked={statusBar} onChange={(on) => { setStatusBar(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-21" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Tab bar</Text>
            <Text fontSize={12}>{tabBar ? "On" : "Off"} · option 22 of 40</Text>
          </View>
          <Switch key="sw-21" checked={tabBar} onChange={(on) => { setTabBar(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-22" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Breadcrumbs</Text>
            <Text fontSize={12}>{breadcrumbs ? "On" : "Off"} · option 23 of 40</Text>
          </View>
          <Switch key="sw-22" checked={breadcrumbs} onChange={(on) => { setBreadcrumbs(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-23" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Minimap</Text>
            <Text fontSize={12}>{minimap ? "On" : "Off"} · option 24 of 40</Text>
          </View>
          <Switch key="sw-23" checked={minimap} onChange={(on) => { setMinimap(on); setChanges(changes + 1); }} />
        </Row>
      </View>
      <View key="sync" gap={4}>
        <Text fontSize={16} fontWeight="medium">Sync</Text>
        <Row key="row-24" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync settings</Text>
            <Text fontSize={12}>{syncSettings ? "On" : "Off"} · option 25 of 40</Text>
          </View>
          <Switch key="sw-24" checked={syncSettings} onChange={(on) => { setSyncSettings(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-25" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync extensions</Text>
            <Text fontSize={12}>{syncExtensions ? "On" : "Off"} · option 26 of 40</Text>
          </View>
          <Switch key="sw-25" checked={syncExtensions} onChange={(on) => { setSyncExtensions(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-26" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync keybindings</Text>
            <Text fontSize={12}>{syncKeybindings ? "On" : "Off"} · option 27 of 40</Text>
          </View>
          <Switch key="sw-26" checked={syncKeybindings} onChange={(on) => { setSyncKeybindings(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-27" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync snippets</Text>
            <Text fontSize={12}>{syncSnippets ? "On" : "Off"} · option 28 of 40</Text>
          </View>
          <Switch key="sw-27" checked={syncSnippets} onChange={(on) => { setSyncSnippets(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-28" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync on metered</Text>
            <Text fontSize={12}>{syncOnMetered ? "On" : "Off"} · option 29 of 40</Text>
          </View>
          <Switch key="sw-28" checked={syncOnMetered} onChange={(on) => { setSyncOnMetered(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-29" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Conflict prompts</Text>
            <Text fontSize={12}>{conflictPrompts ? "On" : "Off"} · option 30 of 40</Text>
          </View>
          <Switch key="sw-29" checked={conflictPrompts} onChange={(on) => { setConflictPrompts(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-30" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Backups</Text>
            <Text fontSize={12}>{backups ? "On" : "Off"} · option 31 of 40</Text>
          </View>
          <Switch key="sw-30" checked={backups} onChange={(on) => { setBackups(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-31" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Sync history</Text>
            <Text fontSize={12}>{syncHistory ? "On" : "Off"} · option 32 of 40</Text>
          </View>
          <Switch key="sw-31" checked={syncHistory} onChange={(on) => { setSyncHistory(on); setChanges(changes + 1); }} />
        </Row>
      </View>
      <View key="privacy" gap={4}>
        <Text fontSize={16} fontWeight="medium">Privacy</Text>
        <Row key="row-32" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Crash reports</Text>
            <Text fontSize={12}>{crashReports ? "On" : "Off"} · option 33 of 40</Text>
          </View>
          <Switch key="sw-32" checked={crashReports} onChange={(on) => { setCrashReports(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-33" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Telemetry</Text>
            <Text fontSize={12}>{telemetry ? "On" : "Off"} · option 34 of 40</Text>
          </View>
          <Switch key="sw-33" checked={telemetry} onChange={(on) => { setTelemetry(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-34" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Spell check online</Text>
            <Text fontSize={12}>{spellCheckOnline ? "On" : "Off"} · option 35 of 40</Text>
          </View>
          <Switch key="sw-34" checked={spellCheckOnline} onChange={(on) => { setSpellCheckOnline(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-35" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Link previews</Text>
            <Text fontSize={12}>{linkPreviews ? "On" : "Off"} · option 36 of 40</Text>
          </View>
          <Switch key="sw-35" checked={linkPreviews} onChange={(on) => { setLinkPreviews(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-36" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Remote images</Text>
            <Text fontSize={12}>{remoteImages ? "On" : "Off"} · option 37 of 40</Text>
          </View>
          <Switch key="sw-36" checked={remoteImages} onChange={(on) => { setRemoteImages(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-37" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Location hints</Text>
            <Text fontSize={12}>{locationHints ? "On" : "Off"} · option 38 of 40</Text>
          </View>
          <Switch key="sw-37" checked={locationHints} onChange={(on) => { setLocationHints(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-38" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Read receipts</Text>
            <Text fontSize={12}>{readReceipts ? "On" : "Off"} · option 39 of 40</Text>
          </View>
          <Switch key="sw-38" checked={readReceipts} onChange={(on) => { setReadReceipts(on); setChanges(changes + 1); }} />
        </Row>
        <Row key="row-39" gap={16} paddingVertical={8} alignItems="center">
          <View flex={1} gap={2}>
            <Text>Typing status</Text>
            <Text fontSize={12}>{typingStatus ? "On" : "Off"} · option 40 of 40</Text>
          </View>
          <Switch key="sw-39" checked={typingStatus} onChange={(on) => { setTypingStatus(on); setChanges(changes + 1); }} />
        </Row>
      </View>
    </View>
  );
}

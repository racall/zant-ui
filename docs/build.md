# 发布构建：zant build

`zant build` 把应用做成可以发给别人的安装文件：编译 → 签名 → 打包 → 公证或提交商店。

```bash
zant build                      # 读项目根目录的 build.config.json，缺的设置在终端里询问
zant build release/ci.json      # 指定配置文件
zant build --yes                # 不提问（CI 用；设置了环境变量 CI 时也不提问）
```

结果放在 `zig-out/dist/`。只想编译用 `zant compile [--release]`，本机测试用的 .app / 程序目录用
`zant package`。

| 平台 | 发布方式（`distribution`） | 结果 |
|---|---|---|
| macOS | `developer-id`：自己分发（默认） | `<标题>-<版本>.dmg`（或 zip），Developer ID 签名，可选 Apple 公证 |
| macOS | `app-store`：Mac App Store | `<标题>-<版本>.pkg`，可选上传到 App Store Connect |
| Windows | `installer`：安装包（默认） | `<标题>-<版本>-setup.exe`（Inno Setup），可签名 |
| Windows | `store`：Microsoft Store | `<标题>-<版本>.msix`，可选用 msstore 命令行提交 |
| Windows | `folder`：免安装 | `<标题>-<版本>-windows.zip` |

目录：

- [第一次运行](#第一次运行)
- [配置文件](#配置文件)
- [macOS：证书与公证](#macos证书与公证)
- [macOS：Mac App Store](#macosmac-app-store)
- [Windows：签名与安装包](#windows签名与安装包)
- [Windows：Microsoft Store](#windowsmicrosoft-store)
- [CI](#ci)
- [常见问题](#常见问题)
- [打包脚本](#打包脚本)

## 第一次运行

在项目目录的终端里运行 `zant build`，配置里缺什么就问什么：

1. 发布方式（见上表）。
2. 签名证书：
   - macOS 列出钥匙串里的证书，推荐的排在前面并标「（推荐）」；也可以选 Ad-hoc，只能在本机测试；
   - Windows 列出证书存储（当前用户 → 个人）里有效的代码签名证书，也可以输入 `f` 用 .pfx 文件，或输入 `0` 不签名。
3. macOS 自己分发：签名完成后问要不要公证，要公证时问用哪个苹果帐户（见下文）；再问 DMG 的背景
   图片，直接回车用 zant 自带的默认背景。
4. Mac App Store：分类、描述文件（自动列出本机匹配的）、安装包证书、是否上传，以及
   App Store Connect API key。
5. Windows：安装包的发布者名称；Microsoft Store 的产品标识；没装 Inno Setup 时提议用 winget 安装。

问完后询问是否保存到配置文件，下次（以及 CI 上）直接用。保存的内容只有证书名称、选项、文件路径
和环境变量名；**密码和密钥内容不会写进去**。配置文件可以提交到仓库供 CI 使用；如果不想公开证书
名称（里面有个人姓名），把它加进 `.gitignore`。

## 配置文件

默认是项目根目录的 `build.config.json`，所有字段都可以省略：

- 字符串写成 `$名字` 或 `${名字}` 时，从环境变量读取（CI 的密钥都这样写）；
- 路径可以用 `~/` 开头，相对路径相对项目根目录；
- 拼错的字段名会报错并停止，不会被悄悄忽略。

```json
{
  "build_number": "$BUILD_NUMBER",
  "macos": {
    "distribution": "developer-id",
    "signing_identity": "Developer ID Application: Your Name (ABCDE12345)",
    "dmg_background": "default",
    "notarize": true,
    "notary": { "profile": "zant-notary" }
  },
  "windows": {
    "distribution": "installer",
    "publisher": "Your Name",
    "signing_identity": "0123456789ABCDEF0123456789ABCDEF01234567"
  }
}
```

### 通用字段

| 字段 | 说明 |
|---|---|
| `build_number` | macOS 的 CFBundleVersion（构建号），默认等于 `app.config.json` 的 `version`。上传 App Store Connect 时，同一个版本再次上传要用更大的构建号；CI 里常写成 `$GITHUB_RUN_NUMBER` |
| `app_store_connect.issuer_id` | App Store Connect API 的 Issuer ID（获取方法见下文） |
| `app_store_connect.key_id` | API key 的 Key ID |
| `app_store_connect.key` | .p8 私钥：文件路径，或文件内容（原文或 base64，CI 用 `$变量`） |

### macOS 字段（`macos`）

| 字段 | 说明 |
|---|---|
| `distribution` | `developer-id`（默认）或 `app-store` |
| `signing_identity` | 钥匙串里签名证书的全名，例如 `Developer ID Application: Your Name (ABCDE12345)`；`none` = Ad-hoc |
| `certificate` / `certificate_password` | CI 用：.p12 证书（路径或 base64 内容）和它的密码。导入一个临时钥匙串，构建结束后删除；`signing_identity` 不写时自动用里面的证书 |
| `format` | `dmg`（默认）或 `zip` |
| `dmg_background` | DMG 窗口背景：PNG 路径、`default`（默认背景）或 `none`（普通窗口），见下文 |
| `notarize` | 是否公证。只有 Developer ID Application 证书签名的应用能公证；不提问时默认不公证 |
| `notary` | 公证用的帐户，三选一：`{"profile": "钥匙串配置名"}`、`{"api_key": true}`、`{"apple_id": "…", "password": "$变量", "team_id": "…"}`（`team_id` 不写时取证书名里的团队 ID） |
| `category` | Mac App Store 分类，例如 `public.app-category.utilities` |
| `provisioning_profile` | Mac App Store 描述文件（.provisionprofile）的路径 |
| `installer_identity` | 给 .pkg 签名的证书全名（Mac Installer Distribution） |
| `upload` | 是否上传到 App Store Connect（先验证再上传） |

### Windows 字段（`windows`）

| 字段 | 说明 |
|---|---|
| `distribution` | `installer`（默认）、`store` 或 `folder` |
| `publisher` | 发布者名称，安装程序和「设置 → 应用」里显示 |
| `signing_identity` | 证书存储里的代码签名证书：指纹（40 位十六进制）或证书主题名称；`none` = 不签名。不提问时默认不签名 |
| `certificate` / `certificate_password` | 改用 .pfx 文件签名：路径或 base64 内容，和它的密码 |
| `timestamp_url` | 时间戳服务器，默认 `http://timestamp.digicert.com` |
| `store.identity_name` / `store.publisher` / `store.publisher_display_name` | Microsoft Store 产品标识里的三个值 |
| `store.upload` | 是否用 msstore 命令行提交 |
| `store.product_id` | Store ID（9 开头的 12 位） |
| `store.tenant_id` / `seller_id` / `client_id` / `client_secret` | msstore 用的 Partner Center 凭据；不写时用之前 `msstore reconfigure` 保存的 |

## macOS：证书与公证

### 需要什么

自己分发、公证、上架都需要付费的 [Apple Developer Program](https://developer.apple.com/programs/) 会员。
没有会员时只能用 Ad-hoc 签名，生成的 DMG 只适合自己测试：别人的 Mac 打开时会被系统拦截。

### 签名证书

| 用途 | 证书类型 | 谁能创建 |
|---|---|---|
| 自己分发（DMG / zip）、公证 | Developer ID Application | 只有团队的帐户持有人（Account Holder） |
| Mac App Store 的应用 | Apple Distribution（较老的帐号可能是 3rd Party Mac Developer Application） | 帐户持有人或管理员 |
| Mac App Store 的 .pkg | Mac Installer Distribution | 帐户持有人或管理员 |

创建方法（二选一）：

- **Xcode**：Xcode → Settings → Accounts，选中 Apple 帐户和团队 → Manage Certificates → 左下角 **+** →
  选证书类型 → Done。证书和私钥直接装进本机钥匙串。
- **开发者网站**：
  1. 在「钥匙串访问」里选 钥匙串访问 → 证书助理 → 从证书颁发机构请求证书，填邮箱和名称，
     选「存储到磁盘」，得到 `.certSigningRequest` 文件；
  2. 登录 [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources/certificates/list)，
     点 Certificates 旁的 **+**，选类型（Developer ID 在 Software → Developer ID 下，选 Developer ID Application），
     上传上一步的文件，Continue，Download 得到 `.cer`；
  3. 双击 `.cer` 装进钥匙串（要在生成请求文件的那台 Mac 上，私钥在那里）。

检查本机有哪些可用证书，`signing_identity` 就写引号里的全名：

```bash
security find-identity -v -p codesigning
```

**团队 ID**（Team ID）是证书名末尾括号里的 10 个字符，也可以在开发者网站的 Membership details 里查到。

### 公证

公证是把签好名的 DMG 交给苹果检查，通过后，别人下载打开时系统不会拦截。zant 提交 DMG（或 zip）、
等待结果（通常 1～10 分钟），通过后把公证票据装订到 DMG 和 .app 上（离线也能验证），再用 Gatekeeper
检查一遍；没通过时打印苹果的检查日志。苹果限制每个团队每天最多公证 75 次。

公证用的帐户有三种写法（`macos.notary`），本机推荐第一种：

**1. 钥匙串配置（`{"profile": "zant-notary"}`）**

把 Apple ID 和 App 专用密码存进钥匙串，配置文件里只写配置名：

1. 生成 App 专用密码：登录 [account.apple.com](https://account.apple.com) → 登录与安全 → App 专用密码 →
   生成（Apple 帐户需要开启双重认证）；
2. 存进钥匙串（会先向苹果验证）：

   ```bash
   xcrun notarytool store-credentials zant-notary --apple-id 你的AppleID --team-id ABCDE12345
   # 按提示输入 App 专用密码
   ```

交互运行 `zant build` 时选「Apple ID + App 专用密码」，zant 会替你运行这条命令，密码由 notarytool
询问，zant 看不到。同一台 Mac 上的其他项目可以共用这个配置名。

**2. App Store Connect API key（`{"api_key": true}`）**

用 `app_store_connect` 里的 API key 公证，适合 CI。**必须是团队密钥**（个人密钥不能用于公证），获取方法见
下文「App Store Connect API key」。

**3. Apple ID 和密码直接写成环境变量**

```json
"notary": { "apple_id": "$APPLE_ID", "password": "$APPLE_APP_PASSWORD", "team_id": "ABCDE12345" }
```

### DMG 窗口

DMG 打开后显示一个带背景的窗口：左边是应用，右边是「应用程序」文件夹，用户拖过去即可安装。

- `dmg_background` 不写或写 `default`：zant 自带的默认背景（浅色渐变和一道箭头）；
- 自己的背景：PNG 的尺寸就是窗口大小（单位是点），建议 640×400；同一目录下放一张两倍大的
  `名字@2x.png`（1280×800）给 Retina 屏幕。应用图标和「应用程序」文件夹的中心分别在宽度的 27% 和 73%、
  高度的 47.5% 处（640×400 时是 (170, 190) 和 (470, 190)），图标 128 点，画背景时把这两处留空；
- `none`：普通窗口。

窗口由「访达」排好后存进 DMG。第一次运行时 macOS 会询问是否允许终端控制「访达」（系统设置 →
隐私与安全性 → 自动化）；不允许或在没有界面的环境里，DMG 照样生成，只是没有背景和图标摆放。
打包时如果已经挂载了同名的磁盘（例如上次打开的 DMG），要先在访达里推出。

## macOS：Mac App Store

上架需要的东西：

1. **App ID**：开发者网站 → Identifiers → **+** → App IDs，选 Explicit，Bundle ID 填 `app.config.json` 的
   `bundle_id`。
2. **App Store Connect 里的应用记录**：[App Store Connect](https://appstoreconnect.apple.com) → App → **+** →
   新建 App，选 macOS、名称、语言、上面的 Bundle ID、SKU。第一次上传前要建好。
3. **证书**：Apple Distribution 和 Mac Installer Distribution（见上文）。
4. **描述文件**：开发者网站 → Profiles → **+** → Distribution 下选 **Mac App Store Connect** → 选 App ID 和
   Apple Distribution 证书 → 命名 → Generate → Download，得到 `.provisionprofile`。双击或放进
   `~/Library/Developer/Xcode/UserData/Provisioning Profiles/`，交互运行时 zant 会自动列出和 Bundle ID 匹配的。
5. **分类**：`category`，常用值 `public.app-category.utilities`（工具）、`productivity`（效率）、
   `developer-tools`（开发者工具）、`graphics-design`（图形和设计），交互时只输入后半段也可以。
   完整列表见苹果文档 [LSApplicationCategoryType](https://developer.apple.com/documentation/bundleresources/information-property-list/lsapplicationcategorytype)。
6. **App Store Connect API key**（要上传时）。

### App Store Connect API key

1. 用帐户持有人或管理员登录 App Store Connect → 用户和访问（Users and Access）→ 集成（Integrations）→
   App Store Connect API → 团队密钥（Team Keys）→ **生成 API 密钥**；
2. 填名称，访问权限选「App 管理」（App Manager）；
3. 页面上方是 **Issuer ID**，列表里是 **Key ID**；
4. **下载 .p8 私钥，只能下载一次**，苹果不保留副本。建议放到 `~/.appstoreconnect/private_keys/`
   （交互运行时 zant 会在这里自动找 `AuthKey_<Key ID>.p8`），**不要放进项目目录或提交到仓库**。

```json
"app_store_connect": { "issuer_id": "…", "key_id": "…", "key": "~/.appstoreconnect/private_keys/AuthKey_XXXXXXXXXX.p8" }
```

### 沙盒与权限

Mac App Store 要求 App Sandbox。`app-store` 方式总是带上沙盒权限，以及由描述文件里的团队 ID 组成的
`com.apple.application-identifier`、`com.apple.developer.team-identifier`。上架前：

- 在 `app.config.json` 设 `"macos": { "sandbox": true }`，用 `zant run` 检查沙盒里各功能是否正常；
- 应用需要的其他权限写在 `app.config.json` 的 `macos.entitlements`，例如联网
  `{"com.apple.security.network.client": true}`、监听端口 `com.apple.security.network.server`、
  读写「下载」文件夹 `com.apple.security.files.downloads.read-write`。

上传后要等苹果处理一段时间，之后在 App Store Connect 的 TestFlight 里能看到这个构建，再在网页上
填写商店信息并提交审核。

## Windows：签名与安装包

### 要不要签名、用什么证书

不签名的安装包可以用，但用户运行时会看到「未知发布者」和 SmartScreen 警告。可选的证书：

| 方式 | 说明 |
|---|---|
| 不签名 | 免费；适合测试或小范围分发 |
| OV 代码签名证书 | 向证书机构购买（每年约 150～300 美元）；2023 年 6 月起私钥必须放在硬件令牌 / HSM 里 |
| EV 代码签名证书 | 更贵；2024 年起和 OV 一样，也要靠下载量积累 SmartScreen 信誉，不再立刻免除警告 |
| Azure Artifact Signing（原 Trusted Signing） | 微软的云签名服务（按月付费，地区有限制）；zant 目前不直接支持，可以改 `windows.ps1` 接入 |
| 自签名证书 | 只适合测试，用户那里一样会被拦截 |

同一个证书长期签名，SmartScreen 信誉会逐渐积累。提交 Microsoft Store 的 MSIX 不需要自己签名（见下文）。

### 用证书存储里的证书（`signing_identity`）

装好的证书（包括 USB 令牌里的）出现在「当前用户 → 个人」：

1. 运行 `certmgr.msc` → 个人 → 证书，双击证书 → 详细信息 → **指纹**；
2. 把指纹去掉空格，写进 `"signing_identity"`（也可以写证书主题名称，指纹更准确）。

交互运行 `zant build` 时会直接列出这里有效的代码签名证书供选择。

### 用 .pfx 文件（`certificate`）

导出：`certmgr.msc` 里右键证书 → 所有任务 → 导出 → 是，导出私钥 → 个人信息交换 - PKCS #12 (.PFX)
（勾选「如果可能，则包括证书路径中的所有证书」）→ 设密码。

```json
"certificate": "~/certs/code-signing.pfx",
"certificate_password": "$ZANT_WIN_CERT_PASSWORD"
```

密码放在环境变量里；交互运行时变量没设置就在终端里询问（不显示、不保存）。签名命令经过 cmd 传递
密码，所以**密码里不要有 `& | < > ^ %` 这些字符**。

zant 签 exe、`zant_skia.dll`、`pdfium.dll`，安装包和卸载程序由 Inno Setup 调用同一个证书签名，都带时间戳
（证书过期后签名仍然有效）。签名工具 signtool 来自 Windows SDK（Visual Studio 的 C++ 工作负载自带），
找不到时用环境变量 `ZANT_SIGNTOOL` 指定。

### 安装包

需要 [Inno Setup](https://jrsoftware.org/isdl.php)（免费）。zant 用 6.7 版测试过（要求 6.3 以上）：

```powershell
winget install --id JRSoftware.InnoSetup -e
```

交互运行时没装会提议替你安装；装在别处时用环境变量 `ZANT_ISCC` 指定 `ISCC.exe`。

安装包的行为：

- 默认装给当前用户，不需要管理员权限，位置 `%LOCALAPPDATA%\Programs\<标题>`；命令行加 `/ALLUSERS`
  装给所有用户；
- 界面跟随系统语言显示中文或英文（简体中文翻译随 zant 提供）；
- 创建开始菜单快捷方式，可选桌面快捷方式，装完可以直接启动；
- 安装、卸载前结束正在运行的这个程序；
- 卸载时删除 `zant.desktop.setLaunchAtLogin` 写的开机启动项，用户数据（`%APPDATA%\<应用标识>`）保留；
- 静默安装：`setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART`。

## Windows：Microsoft Store

1. 在 [Partner Center](https://partner.microsoft.com/dashboard) 注册开发者帐户，新建应用并保留名称；
2. 打开应用 → 产品管理（Product management）→ **产品标识**（Product identity），抄下：
   - Package/Identity/Name → `store.identity_name`
   - Package/Identity/Publisher（`CN=` 开头）→ `store.publisher`
   - Package/Properties/PublisherDisplayName → `store.publisher_display_name`
   - Store ID → `store.product_id`

   这些值区分大小写，要原样复制。
3. `zant build` 生成 `.msix`：版本号是 `version` 加 `.0`（商店要求第四位为 0），图标从
   `assets/icons/app-windows.png` 生成。**商店的 MSIX 不需要自己签名**，微软审核后会重新签名；要签名时
   证书主题必须和 `store.publisher` 完全一致。
4. **第一次提交要在 Partner Center 网页上完成**：新建提交，上传 .msix，填写商店信息。

之后的更新可以用 msstore 命令行（`store.upload: true`）。限制：只能更新已经在网页上首发过的应用，
而且目前只支持免费应用。准备：

1. 安装：`winget install Microsoft.DotNet.DesktopRuntime.9`，再 `winget install "Microsoft Store Developer CLI"`；
2. 关联 Microsoft Entra 应用：Partner Center → 帐户设置 → 用户管理 → **Microsoft Entra 应用程序** → 添加，
   角色选「管理员」（Manager）；点应用名可以看到租户 ID 和客户端 ID，**添加新密钥**得到客户端密钥
   （只显示一次）；
3. 卖家 ID（Seller ID）在 Partner Center 的帐户设置里；
4. 本机可以运行一次 `msstore reconfigure` 保存凭据，之后配置里不用写；CI 上写成环境变量：

```json
"store": {
  "identity_name": "…", "publisher": "CN=…", "publisher_display_name": "…",
  "upload": true, "product_id": "9XXXXXXXXXXX",
  "tenant_id": "$MS_TENANT_ID", "seller_id": "$MS_SELLER_ID",
  "client_id": "$MS_CLIENT_ID", "client_secret": "$MS_CLIENT_SECRET"
}
```

## CI

CI 上运行 `zant build --yes`（GitHub Actions 等会设置环境变量 `CI`，也就不会提问），缺少必需的设置时
停止并指出字段名。证书和密钥放在 CI 的 secret 里，配置文件引用环境变量。

### 准备 secret

把文件转成 base64 文本：

```bash
base64 -i DeveloperID.p12 | pbcopy        # macOS：.p12 证书
base64 -i AuthKey_XXXXXXXXXX.p8 | pbcopy  # .p8 也可以直接粘贴原文
```

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("code-signing.pfx")) | Set-Clipboard   # Windows：.pfx
```

导出 macOS 的 .p12：「钥匙串访问」→ 我的证书，选中证书（连同私钥）→ 文件 → 导出项目 → 个人信息交换（.p12）→
设密码；或在 Xcode 的 Manage Certificates 里右键证书 → Export Certificate。

在 GitHub 仓库的 Settings → Secrets and variables → Actions → New repository secret 里逐个添加，
或用命令行 `gh secret set MAC_CERT_P12_BASE64 < cert.b64`。

### 配置与工作流

```json
{
  "build_number": "$GITHUB_RUN_NUMBER",
  "macos": {
    "signing_identity": "Developer ID Application: Your Name (ABCDE12345)",
    "certificate": "$MAC_CERT_P12_BASE64",
    "certificate_password": "$MAC_CERT_PASSWORD",
    "dmg_background": "default",
    "notarize": true,
    "notary": { "api_key": true }
  },
  "app_store_connect": { "issuer_id": "$ASC_ISSUER_ID", "key_id": "$ASC_KEY_ID", "key": "$ASC_KEY_P8" },
  "windows": {
    "publisher": "Your Name",
    "certificate": "$WIN_PFX_BASE64",
    "certificate_password": "$WIN_PFX_PASSWORD"
  }
}
```

```yaml
# GitHub Actions：在装好 Zig 并构建好 Zant UI（zant 在 PATH 上）之后
- name: Release build (macOS)
  run: zant build build.config.json --yes
  env:
    MAC_CERT_P12_BASE64: ${{ secrets.MAC_CERT_P12_BASE64 }}
    MAC_CERT_PASSWORD: ${{ secrets.MAC_CERT_PASSWORD }}
    ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
    ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
    ASC_KEY_P8: ${{ secrets.ASC_KEY_P8 }}
- uses: actions/upload-artifact@v4
  with:
    name: dist
    path: zig-out/dist/
```

Windows 的步骤相同，环境变量换成 `WIN_PFX_BASE64`、`WIN_PFX_PASSWORD`；要做安装包时先安装 Inno Setup
（`winget install --id JRSoftware.InnoSetup -e`，或用 `ZANT_ISCC` 指向已有的 ISCC.exe）。

从环境变量得到的 .p8 / .p12 / .pfx 内容临时写在 `zig-out/build/`（macOS 上只有当前用户可读），构建结束
或出错时删除；macOS 的临时钥匙串也一样，并恢复原来的钥匙串列表。

## 常见问题

| 提示 | 处理 |
|---|---|
| `build.config.json 解析失败` | JSON 格式错误，或字段名拼错（未知字段会报错） |
| `缺少 macos.signing_identity` 等 | 非交互模式下缺少必需的设置：在终端里运行一次 `zant build` 按提示选择并保存，或写进配置文件 |
| `是环境变量 X，但它没有设置` | 配置里写了 `$X`，运行时要设置这个环境变量 |
| `钥匙串里没有可用的证书「…」` | 证书没装、已过期或缺少私钥；用 `security find-identity -v -p codesigning` 查看，名字要完全一致 |
| `不是 Developer ID Application 证书，跳过公证` | 只有 Developer ID Application 证书签名的应用能公证 |
| `公证没有通过` | 上面会打印苹果的检查日志，按里面的问题修改（常见：缺少加固运行时、签名缺时间戳、包含未签名的可执行文件） |
| 钥匙串配置名找不到 | 先用 `xcrun notarytool store-credentials 名称 …` 创建，名称和 `notary.profile` 一致 |
| `没能设置 DMG 窗口` | 允许终端控制「访达」（系统设置 → 隐私与安全性 → 自动化）；DMG 已经生成，只是没有背景 |
| `已经挂载了名为「…」的磁盘` | 在访达里推出那个磁盘再打包 |
| `找不到 Inno Setup` | `winget install --id JRSoftware.InnoSetup -e`，或设置 `ZANT_ISCC` |
| `签名失败（signtool …）` | 证书不可用（令牌没插、密码错）或时间戳服务器连不上；换 `timestamp_url` 再试 |
| `找不到 signtool.exe` / `makeappx.exe` | 在 Visual Studio Installer 里安装 Windows SDK，或设置 `ZANT_SIGNTOOL` / `ZANT_MAKEAPPX` |
| `版本号 … 不能用于 MSIX` | MSIX 要求 `version` 是 1.2.3 这样的数字 |

## 打包脚本

zant 负责读配置、提问、编译、生成并签名 .app / 程序目录，之后的平台步骤由 `tools/zant/package/` 里的
脚本完成。要调整打包细节（DMG 外观、安装程序选项等）改这些文件即可，不用重新编译 zant：

| 文件 | 内容 |
|---|---|
| `macos.sh` | DMG（背景与图标摆放）/ zip、公证与装订、Gatekeeper 检查、.pkg 与 App Store Connect 上传、CI 临时钥匙串 |
| `dmg-background.png` / `@2x` / `.svg` | 默认的 DMG 背景（SVG 是可编辑的源文件） |
| `windows.ps1` | 列出证书、查找或安装 Inno Setup、签名、安装包、MSIX 与 Microsoft Store 提交、zip |
| `windows-installer.iss` | Inno Setup 安装程序模板（参数由 windows.ps1 用 `/D` 传入） |
| `ChineseSimplified.isl` | Inno Setup 的简体中文界面文字（来自 Inno Setup 仓库） |

zant 用 `ZANT_*` 环境变量把设置传给脚本（每个脚本开头有完整列表），密码和密钥不会出现在命令行和日志里。

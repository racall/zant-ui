# 发布构建：zant build

在应用目录里运行（项目由 `zant create my-app` 创建）。

## 第一次运行

```bash
cd my-app
zant build                      # 读取 build.config.json，终端里询问缺失设置
zant build release/ci.json       # 指定配置文件
zant build --yes                 # 不提问；设置 CI 环境变量或无终端时也不提问
zant package                    # 本机测试：macOS .app / Windows 程序目录
zant compile --release          # 只编译；省略 --release 为调试构建
```

- 在目标平台构建，发布产物在 `target/dist/`；`package` 输出 `target/<标题>.app` 或 `target/<标题>/`（Windows 不签名）。
- TSX 与手写 Rust 共用发布流程；TSX 编译成 Rust，无 JS 运行时，发布包不含热刷新宿主或开发动态库。
- 首次交互选择分发方式、证书、公证 / 商店设置，可保存配置；macOS 列出钥匙串证书与匹配的描述文件，Windows 可选证书存储、`.pfx` 或不签名。
- `app.config.json` 提供标题、`bundle_id`、`version`；Windows exe 属性来自 `description`（空时用标题）、`company`、`copyright`；macOS 使用版权、`macos.category`、`macos.minimum_system_version`（默认 `13.0`）。
- `window.remember` 只影响窗口位置与尺寸（新脚手架 `true`，省略为 `false`）；生成文件无需手改。
- 发布证书不继承 `app.config.json` 的本机签名身份；Windows 安装包仍需单独填 `windows.publisher`，Mac App Store 仍需填构建配置的 `macos.category`（覆盖应用分类）。

| 平台 / `distribution` | 产物（`<标题>-<版本>`） |
|---|---|
| macOS `developer-id`（默认） | `.dmg` 或 `.zip`，Developer ID 签名，可选公证 |
| macOS `app-store` | `.pkg`，可选上传 App Store Connect |
| Windows `installer`（默认） | `-setup.exe`，Inno Setup 安装包，可签名 |
| Windows `store` | `.msix`，可选用 msstore 提交 |
| Windows `folder` | `-windows.zip`，免安装 |

## 配置文件

`build.config.json`（证书名、团队 ID 均为占位符）：

```json
{
  "build_number": "$BUILD_NUMBER",
  "macos": {
    "signing_identity": "Developer ID Application: Your Name (ABCDE12345)", "notarize": true,
    "notary": { "profile": "zant-notary" }
  },
  "windows": { "publisher": "Your Company", "signing_identity": "none" }
}
```

- 所有字段均可省略；非交互模式使用默认值，缺少必需项时停止并指出字段名。
- 字符串整个写成 `$NAME` / `${NAME}` 读取环境变量；路径支持 `~/`，相对路径以项目根目录为准；未知字段报错。
- 密码通过环境变量提供；终端输入的密码不保存，手写的字面值会原样保留。不要提交证书、私钥、密码、token 或真实签名身份；含个人信息的配置可加入 `.gitignore`。

| 通用字段 | 用法 / 默认值 |
|---|---|
| `build_number` | macOS `CFBundleVersion`，默认应用 `version`；同版本再次上传需增大，CI 可用 `$GITHUB_RUN_NUMBER` |
| `app_store_connect.issuer_id` / `app_store_connect.key_id` | App Store Connect API 的 Issuer ID / Key ID |
| `app_store_connect.key` | `.p8` 路径、私钥原文或 base64；CI 用环境变量 |

| `macos` 字段 | 用法 / 默认值 |
|---|---|
| `distribution` | `developer-id`（默认）/ `app-store` |
| `signing_identity` | 钥匙串证书全名；`none` 为 Ad-hoc，仅本机测试，App Store 禁用；非交互时无证书导入则必填 |
| `certificate` / `certificate_password` | `.p12` 路径或 base64、密码（默认空）；导入临时钥匙串，未指定身份时自动选证书 |
| `format` | 自行分发用 `dmg`（默认）/ `zip` |
| `dmg_background` | PNG 路径 / `default`（默认背景）/ `none`（普通窗口） |
| `notarize` | 公证开关；非交互默认 `false`；可公证时交互默认 `true`，仅 Developer ID Application 可用 |
| `notary.profile` | 已保存的 notarytool 钥匙串配置名；与下面两种方式三选一 |
| `notary.api_key` | `true`：使用 `app_store_connect` 团队 API key 公证 |
| `notary.apple_id` / `notary.password` / `notary.team_id` | Apple ID、App 专用密码、团队 ID；团队 ID 默认取证书名末尾括号内容 |
| `category` | App Store 必填，如 `public.app-category.utilities` |
| `provisioning_profile` | App Store `.provisionprofile` 路径 |
| `installer_identity` | `.pkg` 签名证书名；CI 导入证书后可自动选择安装包证书 |
| `upload` | 上传 App Store Connect，先验证再上传；非交互默认 `false`、交互默认 `true` |

| `windows` 字段 | 用法 / 默认值 |
|---|---|
| `distribution` | `installer`（默认）/ `store` / `folder` |
| `publisher` | 安装包发布者，显示在安装程序与「设置 → 应用」；`installer` 必填 |
| `signing_identity` | 当前用户证书存储的 40 位十六进制指纹（去空格）或主题名称；`none` 不签名，非交互默认不签名 |
| `certificate` / `certificate_password` | `.pfx` 路径或 base64、密码（默认空）；优先于证书存储 |
| `timestamp_url` | 默认 `http://timestamp.digicert.com` |
| `store.identity_name` / `store.publisher` / `store.publisher_display_name` | Store 产品标识中的 Name / Publisher / PublisherDisplayName；`store` 必填 |
| `store.upload` | 用 msstore 提交，默认 `false` |
| `store.product_id` | 提交时必填，Store ID（9 开头的 12 位） |
| `store.tenant_id` / `store.seller_id` / `store.client_id` / `store.client_secret` | Partner Center 凭据；提供 `client_secret` 时其余三项必填，否则用 `msstore reconfigure` 已存凭据 |

## macOS：证书与公证

```bash
security find-identity -v -p codesigning  # signing_identity 使用引号里的证书名
xcrun notarytool store-credentials zant-notary --apple-id "$APPLE_ID" --team-id ABCDE12345
# 按提示输入 App 专用密码；构建配置使用 {"profile":"zant-notary"}
```

| 用途 | 证书 | 创建角色 |
|---|---|---|
| 自行分发 / 公证 | Developer ID Application | [帐户持有人](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/) |
| App Store 应用 | Apple Distribution（或旧的 3rd Party Mac Developer Application） | 帐户持有人 / 管理员 |
| App Store 安装包 | Mac Installer Distribution（或旧的 3rd Party Mac Developer Installer） | [帐户持有人 / 管理员](https://developer.apple.com/help/account/certificates/certificates-overview/) |

- 正式分发需 [Apple Developer Program](https://developer.apple.com/programs/) 会员；Ad-hoc 仅用于本机测试。
- 创建证书：Xcode → Settings → Accounts → Manage Certificates → `+`；也可在「钥匙串访问」生成 CSR，再到[开发者网站](https://developer.apple.com/account/resources/certificates/list)申请并在原机器安装 `.cer`（私钥在原机器）。
- App 专用密码：开启双重认证后，在 [Apple 帐户](https://account.apple.com) → 登录与安全 → App 专用密码生成；交互构建也可代存 notarytool 凭据，配置名可跨项目共用。
- 公证还可用 `{"api_key":true}`（[团队密钥](https://developer.apple.com/documentation/AppStoreConnectAPI/creating-api-keys-for-app-store-connect-api)，见下一节），或 `{"apple_id":"$APPLE_ID","password":"$APPLE_APP_PASSWORD","team_id":"ABCDE12345"}`。
- zant 提交 DMG / zip 并等待公证；失败打印苹果日志，通过后装订票据并检查 Gatekeeper。zip 会在 `.app` 装订后重新压缩，票据支持离线验证；苹果建议[每天不超过 75 次](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)。

DMG 背景示例（替换 `build.config.json` 相应字段）：

```json
{ "macos": { "dmg_background": "assets/dmg.png" } }
```

- PNG 尺寸就是窗口点数，建议 640×400；同目录可放 `dmg@2x.png`（1280×800）；PNG 须为 8 位、非隔行，宽 320–2000、高 240–1400。
- 应用 /「应用程序」图标中心在宽度 27% / 73%、高度 47.5% 处，图标 128 点，背景为两处留空；用户拖入「应用程序」安装。
- 允许终端控制「访达」（系统设置 → 隐私与安全性 → 自动化）才能保存背景和布局；无界面 / 无权限仍生成普通 DMG；已挂载的同名磁盘须先推出。

## macOS：Mac App Store

```json
{
  "macos": {
    "distribution": "app-store",
    "signing_identity": "Apple Distribution: Your Company (ABCDE12345)",
    "installer_identity": "Mac Installer Distribution: Your Company (ABCDE12345)",
    "category": "public.app-category.utilities", "provisioning_profile": "~/certs/app.provisionprofile", "upload": true
  },
  "app_store_connect": { "issuer_id": "$ASC_ISSUER_ID", "key_id": "$ASC_KEY_ID", "key": "$ASC_KEY_P8" }
}
```

1. 开发者网站创建 Explicit App ID，Bundle ID 与 `app.config.json` 的 `bundle_id` 一致；首次上传前在 [App Store Connect](https://appstoreconnect.apple.com) 新建 macOS 应用记录。
2. 创建 Mac App Store Connect 类型的描述文件，选择 App ID 与 Apple Distribution 证书；下载后可放 `~/Library/Developer/Xcode/UserData/Provisioning Profiles/`，供交互构建自动查找。
3. 分类可用 `public.app-category.utilities` / `productivity` / `developer-tools` / `graphics-design`（后面三个也须加 `public.app-category.`）；交互输入可省略前缀。[完整分类](https://developer.apple.com/documentation/bundleresources/information-property-list/lsapplicationcategorytype)。
4. 上传用 API key：App Store Connect → 用户和访问 → 集成 → App Store Connect API → 团队密钥，由帐户持有人 / 管理员创建，权限选「App 管理」。保存 Issuer ID、Key ID 与只能下载一次的 `.p8`；建议存 `~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8`，交互时会查找。
5. `app-store` 自动启用沙盒及应用 / 团队标识权限。先在 `app.config.json` 设置 `macos.sandbox: true`，用 `zant run --no-hot` 检查（默认热刷新宿主不启用沙盒）；其他权限放 `macos.entitlements`：联网 `com.apple.security.network.client`、监听端口 `com.apple.security.network.server`、下载目录读写 `com.apple.security.files.downloads.read-write`，值均为 `true`。
6. 上传处理完成后在 TestFlight 查看；商店信息与提交审核仍在 App Store Connect 网页完成。只生成 `.pkg` 可设 `upload: false`。

## Windows：签名与安装包

```json
{
  "windows": {
    "publisher": "Your Company", "certificate": "~/certs/code-signing.pfx",
    "certificate_password": "$WIN_PFX_PASSWORD"
  }
}
```

```powershell
winget install --id JRSoftware.InnoSetup -e
# 静默安装（把文件名换成实际产物）：
.\setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

- 可用 OV / EV 代码签名证书；自签名仅用于测试。不签名显示「未知发布者」，签名后仍可能出现 SmartScreen 提示；云签名尚未直接支持，接入需改打包脚本。
- `.pfx`：在 `certmgr.msc` 导出证书并包含私钥、证书链，设密码；环境变量未设置时交互询问且不保存。安装包签名经 cmd 传密码，**不要含 `& | < > ^ %`**。
- zant 签 exe 与 `pdfium.dll`，Inno Setup 签安装包和卸载程序，均带时间戳，证书过期后签名仍有效；SDK 提供 signtool，非默认位置用 `ZANT_SIGNTOOL`。
- 模板面向 Inno Setup 6.3+（不检查版本号）；未安装时交互构建可用 winget 安装，非默认位置用 `ZANT_ISCC` 指向 `ISCC.exe`。
- 默认安装到 `%LOCALAPPDATA%\Programs\<标题>`，无需管理员权限；安装命令加 `/ALLUSERS` 为全用户安装。
- 界面支持中英文，跟随系统语言；创建开始菜单快捷方式，可选桌面快捷方式和安装后启动；安装 / 卸载前结束正在运行的应用。
- 卸载删除框架开机启动 API 写入的启动项，保留 `%APPDATA%\<应用标识>` 用户数据。

## Windows：Microsoft Store

```json
{
  "windows": {
    "distribution": "store", "signing_identity": "none", "store": {
      "identity_name": "YourPackageName", "publisher": "CN=YourPublisher",
      "publisher_display_name": "Your Company", "upload": true, "product_id": "9XXXXXXXXXXX",
      "tenant_id": "$MS_TENANT_ID", "seller_id": "$MS_SELLER_ID",
      "client_id": "$MS_CLIENT_ID", "client_secret": "$MS_CLIENT_SECRET"
    }
  }
}
```

1. 在 [Partner Center](https://partner.microsoft.com/dashboard) 注册开发者帐户、新建应用并保留名称；产品管理 → 产品标识中原样复制 Name、Publisher（`CN=` 开头）、PublisherDisplayName 与 Store ID，区分大小写。
2. 提供 `assets/icons/app-windows.png`，zant 生成 MSIX 图标；推荐应用版本 `1.2.3`，生成 `1.2.3.0`（第四位固定 0）。MSIX 用 Windows SDK 的 makeappx，路径可用 `ZANT_MAKEAPPX` 指定。
3. 商店包无需自行签名，微软会重新签名；若自行签名，证书主题必须与 `store.publisher` 完全一致。
4. [首次发布](https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/overview)须在 Partner Center 网页上传 `.msix`、填商店信息并通过审核；发布后可设 `store.upload: true` 提交更新（CLI 目前只支持免费产品）。
5. 自动提交先装 `winget install "Microsoft Store Developer CLI"`，按 [CLI 文档](https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/overview)准备运行时；在 Partner Center 帐户设置 → 用户管理关联 Microsoft Entra 应用，授予 Manager，取得租户 / 客户端 ID 和只显示一次的客户端密钥；卖家 ID 在帐户设置中。
6. 本机可先 `msstore reconfigure` 保存凭据，再省略配置的四个凭据字段；只生成 MSIX 则设 `upload: false`，无需 `product_id` 或提交凭据。

## CI

`build.config.json`（在各自平台运行 `zant build build.config.json --yes`）：

```json
{
  "build_number": "$GITHUB_RUN_NUMBER",
  "macos": {
    "certificate": "$MAC_CERT_P12_BASE64", "certificate_password": "$MAC_CERT_PASSWORD",
    "dmg_background": "none", "notarize": true, "notary": { "api_key": true }
  },
  "app_store_connect": { "issuer_id": "$ASC_ISSUER_ID", "key_id": "$ASC_KEY_ID", "key": "$ASC_KEY_P8" },
  "windows": {
    "publisher": "Your Company", "certificate": "$WIN_PFX_BASE64", "certificate_password": "$WIN_PFX_PASSWORD"
  }
}
```

```yaml
# GitHub Actions：已有固定版本 Rust 工具链、Zant 依赖与 PATH 中的 zant；macOS runner
- run: zant build build.config.json --yes
  env:
    MAC_CERT_P12_BASE64: ${{ secrets.MAC_CERT_P12_BASE64 }}
    MAC_CERT_PASSWORD: ${{ secrets.MAC_CERT_PASSWORD }}
    ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
    ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
    ASC_KEY_P8: ${{ secrets.ASC_KEY_P8 }}
- uses: actions/upload-artifact@v4
  with:
    name: dist
    path: target/dist/
```

- macOS 在「钥匙串访问 → 我的证书」连同私钥导出 `.p12` 并设密码（或 Xcode → Manage Certificates → Export Certificate）；`base64 -i DeveloperID.p12 | pbcopy` 转成 secret，`.p8` 可用原文或同法转 base64。
- Windows：`[Convert]::ToBase64String([IO.File]::ReadAllBytes("code-signing.pfx")) | Set-Clipboard`；工作流传 `WIN_PFX_BASE64` / `WIN_PFX_PASSWORD`，制作安装包前安装 Inno Setup 或设 `ZANT_ISCC`。
- 在 GitHub Settings → Secrets and variables → Actions 添加 secret，或 `gh secret set MAC_CERT_P12_BASE64 < cert.b64`；工具链版本见项目 `rust-toolchain.toml`。
- 临时密钥文件在 `target/build/`（macOS 仅当前用户可读），成功或出错后清理；临时钥匙串也会删除并恢复原搜索列表。

## 常见问题

| 提示 / 问题 | 处理 |
|---|---|
| 配置解析失败 / 缺少字段 / 环境变量未设置 | 检查 JSON、字段拼写与环境变量；终端运行 `zant build` 补齐并保存 |
| 钥匙串无可用证书 | 检查名称、有效期和私钥；运行 `security find-identity -v -p codesigning` |
| 非 Developer ID，跳过公证 | 改用 Developer ID Application；显式要求公证时会报错 |
| 公证失败 / 配置名找不到 | 按苹果日志修改签名、加固运行时、时间戳等问题；用 `notarytool store-credentials` 创建同名配置 |
| DMG 无背景 / 同名磁盘已挂载 | 允许终端控制访达；先推出同名磁盘 |
| 找不到 Inno Setup / signtool / makeappx | 安装 Inno Setup / Windows SDK，或设 `ZANT_ISCC` / `ZANT_SIGNTOOL` / `ZANT_MAKEAPPX` |
| signtool 签名失败 | 检查令牌、证书、密码、网络；必要时更换 `timestamp_url` |
| MSIX 版本号不可用 | 应用 `version` 使用 `1.2.3` 这样的数字 |

## 打包脚本

需自定义分发行为时修改 [scripts/package/](../scripts/package/)；脚本、背景和安装模板改后无需重编 zant，CLI Rust 改后需重编。

| 文件 | 用途 |
|---|---|
| `macos.sh` | DMG / zip、公证装订、Gatekeeper、PKG / 上传、CI 钥匙串 |
| `dmg-background.png` / `dmg-background@2x.png` / `dmg-background.svg` | 默认背景与可编辑源文件 |
| `windows.ps1` / `windows-installer.iss` / `ChineseSimplified.isl` | 签名、Inno Setup 模板、简体中文翻译、MSIX / 上传、zip |

配置入口：[distribute.rs](../crates/zant-cli/src/distribute.rs)。CLI 用 `ZANT_*` 传设置（脚本开头列出），不主动打印密钥；系统工具仍可能通过参数接收凭据。

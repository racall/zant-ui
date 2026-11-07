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
| macOS | `developer-id`：自己分发（默认） | Developer ID 签名的 DMG（或 zip），可选 Apple 公证 |
| macOS | `app-store`：Mac App Store | 签名的 .pkg，可选上传到 App Store Connect |
| Windows | `installer`：安装包（默认） | `<标题>-<版本>-setup.exe`（Inno Setup），可签名 |
| Windows | `store`：Microsoft Store | `.msix`，可选用 msstore 命令行提交 |
| Windows | `folder`：免安装 | 程序目录的 zip |

## 交互与保存

在终端里运行时，配置里缺什么就问什么：

- 发布方式；
- 签名证书：macOS 列出钥匙串里的证书（推荐的排在前面，也可以选 Ad-hoc 只做本机测试），
  Windows 列出证书存储（当前用户 → 个人）里的代码签名证书，或用 .pfx 文件，也可以不签名；
- macOS 签名完成后问要不要公证，要公证时问用哪个苹果帐户（见下文）；
- DMG 的背景图片：直接回车用 zant 自带的默认背景（一道从应用指向「应用程序」文件夹的箭头）；
- Mac App Store：分类、描述文件（自动找出本机匹配的）、安装包证书、是否上传、
  App Store Connect API key（Issuer ID、Key ID、.p8 文件）；
- Windows：发布者名称；Microsoft Store 的产品标识；没装 Inno Setup 时提议用 winget 安装。

问完后询问是否保存到配置文件，下次（以及 CI 上）直接用。**配置文件里只有证书名称、选项和环境变量名**，
密码和密钥内容从不写进去，可以提交到仓库。

## 配置文件

所有字段都可以省略。字符串写成 `$名字`（或 `${名字}`）时从环境变量读取，适合 CI 的密钥；
路径可以用 `~/`，相对路径相对项目根目录。拼错的字段名会报错。

```json
{
  "build_number": "$BUILD_NUMBER",
  "macos": {
    "distribution": "developer-id",
    "signing_identity": "Developer ID Application: Your Name (TEAMID)",
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

### 通用

| 字段 | 说明 |
|---|---|
| `build_number` | macOS 的 CFBundleVersion；App Store Connect 要求同一版本每次上传都更大。默认等于 `app.config.json` 的 `version` |
| `app_store_connect.issuer_id` / `key_id` | App Store Connect API key（App Store Connect → 用户和访问 → 集成） |
| `app_store_connect.key` | .p8 文件路径，或文件内容（CI 用 `$变量`，内容可以是原文或 base64） |

### macOS（`macos`）

| 字段 | 说明 |
|---|---|
| `distribution` | `developer-id`（默认）或 `app-store` |
| `signing_identity` | 钥匙串里的签名证书全名（`security find-identity -v -p codesigning` 显示的名字）；`none` = Ad-hoc |
| `certificate` / `certificate_password` | CI：.p12 证书（路径或 base64 内容）和密码，导入临时钥匙串，构建结束后删除 |
| `format` | `dmg`（默认）或 `zip` |
| `dmg_background` | DMG 窗口的背景：PNG 路径、`default`（默认背景）或 `none`（普通窗口）。图片尺寸就是窗口大小（点），建议 640×400；同目录的 `名字@2x.png` 用于 Retina 屏幕。应用图标和「应用程序」文件夹的中心放在宽度的 27% 和 73%、高度的 47.5% 处，自己画背景时按这个位置留空 |
| `notarize` | 是否公证（只有 Developer ID Application 证书能公证）。不提问时默认不公证 |
| `notary` | 公证用的帐户，三选一：`{"profile": "名称"}`、`{"api_key": true}`、`{"apple_id": …, "password": "$变量", "team_id": …}` |
| `category` | App Store 分类，如 `public.app-category.utilities` |
| `provisioning_profile` | Mac App Store 描述文件（.provisionprofile） |
| `installer_identity` | 签 .pkg 的证书：3rd Party Mac Developer Installer / Mac Installer Distribution |
| `upload` | 是否上传到 App Store Connect（先验证再上传） |

公证帐户：

- `profile`：用 `xcrun notarytool store-credentials` 存在钥匙串里的配置。交互时选
  「Apple ID + App 专用密码」，zant 会调用它，密码由 notarytool 询问并存进钥匙串，zant 看不到；
- `api_key`：用 `app_store_connect` 的 API key，适合 CI；
- `apple_id`：Apple ID + App 专用密码，密码写成 `$变量`。

公证的是 DMG（或 zip），通过后把票据装订到 DMG 和 .app 上，再用 Gatekeeper 检查一遍。

DMG 窗口（背景、图标大小和位置）由「访达」排好后存进 DMG。第一次运行时 macOS 会询问是否允许终端
控制「访达」；不允许或在无界面的环境里，DMG 照样生成，只是窗口没有背景和图标摆放。打包时如果
已经挂载了同名的磁盘，要先推出。

Mac App Store 要求 App Sandbox：这种发布方式总是带上沙盒权限，以及描述文件里的团队 ID 组成的
`application-identifier`、`team-identifier`。上架前在 `app.config.json` 设 `macos.sandbox: true`，
用 `zant run` 检查沙盒里各功能是否正常；应用需要的其他权限写在 `app.config.json` 的
`macos.entitlements`（如 `{"com.apple.security.network.client": true}`）。

### Windows（`windows`）

| 字段 | 说明 |
|---|---|
| `distribution` | `installer`（默认）、`store` 或 `folder` |
| `publisher` | 发布者名称，安装程序和「设置 → 应用」里显示 |
| `signing_identity` | 证书存储里的代码签名证书：指纹（40 位）或主题名称；`none` = 不签名。不提问时默认不签名 |
| `certificate` / `certificate_password` | 用 .pfx 文件签名（路径或 base64 内容）和密码 |
| `timestamp_url` | 时间戳服务器，默认 `http://timestamp.digicert.com` |
| `store.identity_name` / `publisher` / `publisher_display_name` | Partner Center → 产品管理 → 产品标识 里的三个值 |
| `store.upload` / `product_id` | 用 msstore 命令行提交，`product_id` 是 9 开头的 Store ID |
| `store.tenant_id` / `seller_id` / `client_id` / `client_secret` | Partner Center 的 Azure AD 应用凭据；不填时用之前 `msstore reconfigure` 保存的 |

签名时签 exe、`zant_skia.dll`、`pdfium.dll`，安装包和卸载程序由 Inno Setup 调用同一个证书签名；
.pfx 的密码只放在环境变量里，不出现在命令行输出中。

安装包：

- 默认装给当前用户（不需要管理员权限），位置 `%LOCALAPPDATA%\Programs\<标题>`；
  命令行加 `/ALLUSERS` 装给所有用户；
- 界面按系统语言显示中文或英文（简体中文翻译随 zant 提供）；
- 创建开始菜单快捷方式，可选桌面快捷方式，装完可以直接启动；
- 安装、卸载前结束正在运行的这个程序；
- 卸载时删除 `zant.desktop.setLaunchAtLogin` 写的开机启动项，用户数据（`%APPDATA%\<应用标识>`）保留；
- 需要 [Inno Setup](https://jrsoftware.org/isinfo.php) 6.3 以上（免费）：
  `winget install JRSoftware.InnoSetup`，或用环境变量 `ZANT_ISCC` 指定 ISCC.exe。

MSIX 用 Windows SDK 的 makeappx（Visual Studio 的 C++ 工作负载自带），版本号是 `version` 加 `.0`，
图标从 `assets/icons/app-windows.png` 生成。提交商店时可以不签名（微软会签名）；签名时证书主题
要和 `store.publisher` 完全一致。

## CI

CI 上运行 `zant build --yes`（或设置环境变量 `CI`），不会提问；缺少必需的设置时停止并指出字段名。
证书和密钥放在 CI 的 secret 里，配置文件引用环境变量：

```json
{
  "build_number": "$GITHUB_RUN_NUMBER",
  "macos": {
    "signing_identity": "Developer ID Application: Your Name (TEAMID)",
    "certificate": "$MAC_CERT_P12_BASE64",
    "certificate_password": "$MAC_CERT_PASSWORD",
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
# GitHub Actions 片段
- run: zant build build.config.json --yes
  env:
    MAC_CERT_P12_BASE64: ${{ secrets.MAC_CERT_P12_BASE64 }}
    MAC_CERT_PASSWORD: ${{ secrets.MAC_CERT_PASSWORD }}
    ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
    ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
    ASC_KEY_P8: ${{ secrets.ASC_KEY_P8 }}
```

从环境变量得到的 .p8 / .p12 / .pfx 内容临时写在 `zig-out/build/`（只有当前用户可读），
构建结束或出错时删除；临时钥匙串也一样。

## 打包脚本

zant 负责读配置、提问、编译、生成并签名 .app / 程序目录，之后的平台步骤由
`tools/zant/package/` 里的脚本完成，要调整打包细节（DMG 外观、安装程序选项等）改这些文件即可，
不用重新编译 zant：

| 文件 | 内容 |
|---|---|
| `macos.sh` | DMG（背景与图标摆放）/ zip、公证与装订、Gatekeeper 检查、.pkg 与 App Store Connect 上传、CI 临时钥匙串 |
| `dmg-background.png` / `@2x` / `.svg` | 默认的 DMG 背景（SVG 是可编辑的源文件） |
| `windows.ps1` | 列出证书、查找或安装 Inno Setup、签名、安装包、MSIX 与 Microsoft Store 提交、zip |
| `windows-installer.iss` | Inno Setup 安装程序模板（参数由 windows.ps1 用 `/D` 传入） |
| `ChineseSimplified.isl` | Inno Setup 的简体中文界面文字（来自 Inno Setup 仓库） |

zant 用 `ZANT_*` 环境变量把设置传给脚本（每个脚本开头有列表），密码和密钥不会出现在命令行里。

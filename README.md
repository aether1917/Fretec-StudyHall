# Fretec StudyHall

番茄钟 + 学习流的跨端应用：Rust + Tauri v2 一套代码构建 Windows / Linux / Android。
纯黑单色界面，右上角常驻翻页钟胶囊，可拖拽排序的学习流，内置播放器（画质 / 倍速 / 进度续播），
B 站与网易云搜索、扫码 / 手机验证码 / 密码登录，局域网倒计时同步，桌面托盘与 Android 常驻通知。

## 功能

- **计时**：专注 25 分钟 / 短休息 5 分钟循环，阶段自动切换；胶囊点击纯黑圆形扩散展开翻页钟；暂停 / 继续 / 结束。
- **学习流**：B 站视频与网易云音乐条目，长按拖动排序、右滑删除、封面进度条，空态引导。
- **播放器**：全屏覆盖层，B 站 DASH 双轨（视频 + 音频）或网易云单轨音频，360P / 720P / 1080P、0.5x–2.0x 倍速、进度续播（±2s），关闭后迷你播放条停在底栏上方。
- **搜索**：哔哩哔哩（WBI 签名）与网易云（weapi 加密），500ms 防抖、分页、已添加标记；计时中锁定添加入口。
- **登录**：两家均支持二维码与手机验证码；Android 额外支持手机号 + 密码；B 站人机验证在应用内完成；登录态过期提示条。
- **局域网**：UDP 广播发现 + TCP 长连接，仅同步倒计时状态，最多 4 台，计时中双向禁止新连接。
- **系统集成**：Windows / Linux 托盘（关闭窗口隐藏、tooltip 显示阶段与剩余时间、菜单退出）；Android 前台服务常驻通知（系统计时器递减、暂停 / 继续 / 结束、点击回应用）。
- **数据**：学习流、顺序、播放进度、设备名、登录凭据由 Rust 写入应用数据目录的 JSON；凭据不返回前端、不写日志。

## 目录结构

```
index.html / captcha.html      前端入口（Vite 多页）
src/                           TypeScript 前端（原生 TS + M3 令牌）
src/styles/tokens.css          唯一色值来源（--md-sys-*）
src-tauri/                     Rust 后端（Tauri v2）
src-tauri/src/bili/            B 站 WBI / 搜索 / playurl / 扫码 / 短信 / 密码登录
src-tauri/src/netease/         网易云 weapi / 搜索 / 播放地址 / 登录
src-tauri/src/lan/             局域网发现与倒计时同步
src-tauri/src/proxy.rs         127.0.0.1 视频代理（补 Referer / UA、Range 透传）
src-tauri/src/timer.rs         计时状态机（Rust 为唯一真源）
plugins/tauri-plugin-studyhall-native/   Android 前台服务插件（Rust + Kotlin）
scripts/                       打包 / 工具链 / Gitee 绑定 / 令牌检查
out/                           安装包产物（不进 git）
```

## 环境要求

| 平台 | 依赖 |
| --- | --- |
| 通用 | Node.js ≥ 20、pnpm ≥ 9、Rust stable（rustup） |
| Windows | MSVC Build Tools 2022（VCTools + Windows SDK）、WebView2 Runtime |
| Linux | `libwebkit2gtk-4.1-dev`、`libappindicator3-dev`、`librsvg2-dev`、`patchelf`、GTK3 |
| Android | JDK 17、Android SDK（platform 35 / build-tools 35）、NDK r27、Rust Android targets |

本机（Windows 10）已验证的安装方式：

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
winget install Rustlang.Rustup -e
rustup default stable-x86_64-pc-windows-msvc
pnpm install
```

Android 额外环境（JDK 17 + SDK/NDK + Rust targets）：

```powershell
winget install EclipseAdoptium.Temurin.17.JDK -e
powershell -ExecutionPolicy Bypass -File scripts/setup-android.ps1
```

## 开发与构建

```powershell
pnpm install
pnpm dev                 # 前端 dev server（Tauri dev 时会自动使用）
pnpm tauri dev           # 桌面开发模式
pnpm typecheck           # TypeScript 检查
pnpm build               # 前端生产构建（含 captcha 页）
node scripts/check-tokens.mjs   # 设计令牌检查：tokens.css 之外不允许色值
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

### Windows 安装包

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

脚本会先预热 NSIS 工具链（`%LOCALAPPDATA%\tauri\NSIS`，经 gh-proxy 下载并校验 SHA1），
再执行 `pnpm tauri build --bundles nsis`，把安装包复制到 `out/`。

### Android APK

```powershell
$env:JAVA_HOME="C:\Program Files\Eclipse Adoptium\jdk-17..."
$env:ANDROID_HOME="$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME="$env:ANDROID_HOME\ndk\27.2.12479018"
powershell -ExecutionPolicy Bypass -File scripts/package-android.ps1
```

产物复制到 `out/`。首次构建前需要初始化 Android 工程：

```powershell
pnpm tauri android init
```

### Linux

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev
pnpm install && pnpm tauri build --bundles deb,appimage
```

CI（`.github/workflows/build.yml`）同时构建 Windows / Linux / Android 产物。

## 第三方依赖清单

### 前端（package.json）

| 依赖 | 用途 |
| --- | --- |
| `@tauri-apps/api` | 前端调用 Rust 命令与事件 |
| `@tauri-apps/cli` | Tauri 开发 / 构建 CLI |
| `qrcode` | 登录二维码生成 |
| `vite` | 前端构建与 dev server |
| `typescript` | 类型检查 |

字体：`Fredoka`（SIL OFL 1.1，见 `public/fonts/OFL.txt`），仅翻页钟数字使用。

### Rust（Cargo.toml）

| crate | 用途 |
| --- | --- |
| `tauri` | 应用框架、窗口、托盘、命令与事件 |
| `tokio` | 异步运行时、局域网 TCP/UDP、计时循环 |
| `reqwest` | 第三方 HTTP 请求（B 站 / 网易云） |
| `axum` | 本地视频代理 HTTP 服务 |
| `serde` / `serde_json` | 数据模型与本地 JSON 持久化 |
| `md5` | WBI 签名、网易云密码摘要 |
| `sha2` | RSA-OAEP 摘要、SHA1 校验辅助 |
| `aes` / `cbc` | 网易云 weapi AES-128-CBC |
| `rsa` / `num-bigint` | 网易云无填充 RSA、B 站 RSA-OAEP |
| `base64` / `urlencoding` / `percent-encoding` | 编码 |
| `rand` | 随机密钥、设备 id |
| `uuid` | 条目 / 代理 key |
| `bytes` | 代理流式响应 |
| `http` | 代理头处理 |
| `thiserror` | 统一错误类型 |

### 系统工具

| 工具 | 用途 |
| --- | --- |
| NSIS 3.13 + `nsis_tauri_utils.dll` | Windows 安装包（Tauri bundler 缓存） |
| Android SDK / NDK / JDK 17 | Android 构建 |

## 数据位置

Windows：`%APPDATA%\com.fretec.studyhall\`，包含 `flow.json`（学习流与进度）、`device.json`（设备名）、
`auth.json`（登录凭据，仅 Rust 读写）、`settings.json`。Android 使用应用私有目录。

## 已知限制与未验证项

- B 站 WBI / 网易云 weapi 均为非官方接口，存在风控、限流与改版风险；接口失败会给出中文错误，不崩溃。
- B 站短信 / 密码登录依赖极验人机验证组件（`https://static.geetest.com`），需要真实账号验证；组件加载失败时可用二维码登录。
- 本机无第二台设备与 Android 真机，局域网双机同步与 Android 通知行为仅通过代码与单元测试验证。
- Linux 安装包由 CI 构建，本机未验证。
- 部分 Linux 发行版 WebView 缺少 H.264 解码，B 站视频可能无法播放。

## 许可证

字体 Fredoka 使用 SIL OFL 1.1；其余代码未声明许可证。

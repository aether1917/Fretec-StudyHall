# Fretec StudyHall

一个番茄钟加学习流的桌面/移动应用，用 Rust + Tauri v2 写，一套代码构建 Windows、Linux 和 Android。界面是纯黑单色，右上角是翻页钟胶囊，底部是学习流和设置两个页面，学习流里可以放 B 站视频和网易云音乐，点开就能在应用内播放。

现在做出来的东西：

- 计时：专注 25 分钟、休息 5 分钟循环，自动切换。点胶囊会从胶囊位置扩散出一块纯黑，铺满后显示翻页钟；再点缩小会收回去。暂停、继续、结束后状态同步到托盘和 Android 通知。
- 学习流：卡片可以长按拖动排序，右滑超过 30% 会停在滑开状态，点删除移除。卡片底部有播放进度条。
- 播放器：B 站视频走本地代理，补上 Referer 和 UA；画质 360P / 720P / 1080P，倍速 0.5x 到 2.0x；返回后播放器收到迷你条里，进度按秒记在本地，下次打开接着播。
- 搜索：顶栏左上角打开搜索层，来源在哔哩哔哩和网易云之间切换。B 站请求带 WBI 签名，网易云走 weapi 加密；500ms 防抖，滚动分页，最多 100 条。计时中不能添加。
- 登录：两家都支持二维码和手机验证码，Android 额外有手机号加密码。B 站人机验证在应用内的独立窗口里完成。
- 局域网：UDP 广播发现，TCP 长连接，只同步倒计时状态，最多 4 台。计时进行中不允许新建连接，学习流和播放进度不同步。
- 托盘和通知：Windows/Linux 关窗口后留在托盘，单击显示或隐藏，菜单里有退出；Android 用前台服务维护常驻通知。

## 目录

```
src/                     前端，TypeScript，没有框架
src/styles/tokens.css    颜色、圆角、字号等设计令牌
src-tauri/               Rust 侧和 Tauri 配置
src-tauri/src/bili/      B 站搜索、取流、扫码/短信/密码登录
src-tauri/src/netease/   网易云 weapi、搜索、播放地址、登录
src-tauri/src/lan/       局域网发现和倒计时同步
src-tauri/src/proxy.rs   127.0.0.1 上的视频代理
src-tauri/src/timer.rs   计时状态机
plugins/                 Android 前台服务插件（Kotlin）
scripts/                 打包、初始化环境、Gitee 绑定、颜色令牌检查
out/                     安装包输出目录
```

## 构建

需要 Node 20 以上、pnpm、Rust stable。Windows 还要 MSVC Build Tools 和 WebView2；Linux 要 webkit2gtk 4.1、libappindicator3、librsvg2、patchelf；Android 要 JDK 17、Android SDK 36、NDK r27 和四个 Android target。

前端：

```powershell
pnpm install
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Windows 安装包：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

脚本会先把 NSIS 3.13 和插件放到 `%LOCALAPPDATA%\tauri\NSIS`（本机访问不了 GitHub，走 gh-proxy 下载并校验 SHA1），再执行 `pnpm tauri build --bundles nsis`，最后把安装包复制到 `out/`。

Android：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup-android.ps1
pnpm tauri android init
powershell -ExecutionPolicy Bypass -File scripts/package-android.ps1
```

这台机器没开 Windows 开发者模式，Tauri CLI 往 jniLibs 放 .so 时要用符号链接，会失败。所以我改成手动跑一遍：先 `cargo build --target aarch64-linux-android --release --lib`，把 `.so` 复制进 `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/`，再把 `buildSrc` 里 `BuildTask.kt` 的执行体改成空实现，直接 `gradle assembleArm64Release`。签名配置写在 `gen/android/app/build.gradle.kts` 里，读的是 `keystore.properties`。这些文件都在 `gen/` 下，不进 git；重新 init 后要重新改一遍。开启开发者模式的机器直接 `pnpm tauri android build --apk` 就行。

Linux：

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev
pnpm install && pnpm tauri build --bundles deb,appimage
```

`.github/workflows/build.yml` 里有一套三端的构建流程，本机没有 Linux 环境，没跑过。

另外，这台机器装 VS Build Tools 时被 Microsoft CDN 卡住了，所以桌面构建用的是另一种办法：xwin 下载 MSVC CRT 和 Windows SDK，rustup 自带的 `rust-lld` 改名成 `lld-link.exe` 当链接器，`rc.exe` 从 NuGet 的 `Microsoft.Windows.SDK.BuildTools` 里取，`scripts/dev-env.ps1` 负责设置这些环境变量。`src-tauri/vendor/vswhom-sys` 是为了绕开 `embed-resource` 对 C++ 编译器的依赖写的空实现。正常装了 VS Build Tools 的机器不需要这些。

`src-tauri/build.rs` 没有用 tauri-build 默认的 manifest 嵌入，因为那个只链接到二进制目标，单元测试里链接 tauri 会找不到 Common Controls v6，一启动就 `STATUS_ENTRYPOINT_NOT_FOUND`。现在改成对所有 Windows 链接目标统一用 `rustc-link-arg` 嵌入 `windows-app-manifest.xml`。

## 依赖

前端：`@tauri-apps/api` 调 Rust 命令和事件；`@tauri-apps/cli` 是构建工具；`qrcode` 生成登录二维码；`vite` 构建；`typescript` 做类型检查。

Rust：`tauri` 提供窗口、托盘、命令和事件；`tokio` 跑异步任务、局域网和计时循环；`reqwest` 请求第三方接口（桌面用系统 TLS，Android 用 rustls）；`axum` 起本地代理；`serde`/`serde_json` 处理数据和本地 JSON；`md5` 做 WBI 签名和网易云密码摘要；`aes`/`cbc` 做 weapi 加密；`rsa`/`num-bigint` 做网易云裸 RSA 和 B 站 RSA-OAEP；`base64`、`urlencoding`、`percent-encoding` 做编码；`uuid` 生成条目和代理 key；`thiserror` 统一错误。

字体是 Fredoka（SIL OFL 1.1，见 `public/fonts/OFL.txt`），只用在翻页钟数字上。

## 数据

Windows 存在 `%APPDATA%\com.fretec.studyhall\`，Android 在应用私有目录。`flow.json` 存学习流、排序和播放进度，`device.json` 存设备名，`auth.json` 存登录 cookie。凭据只在 Rust 侧读写，不返回前端，也不写日志。

## 本机验证情况

Windows 上跑过：启动、两个页面切换、胶囊展开和收回、开始/暂停/继续、B 站真实搜索和添加、卡片打开播放器并播放、返回后迷你条出现、`flow.json` 里进度正常写入。`cargo test` 50 个测试通过，`cargo clippy -D warnings` 通过，`pnpm typecheck` 和 `pnpm build` 通过，`node scripts/check-tokens.mjs` 通过。产物是 `out/Fretec StudyHall_0.0.1-beta.1_x64-setup.exe` 和 `out/Fretec-StudyHall-0.0.1-beta.1-arm64.apk`。

没条件验证的：扫码、短信、密码登录和极验验证（需要真实账号）；局域网两台设备之间的发现和同步（只有代码和单元测试）；Android 真机上的通知和权限弹窗；托盘悬停提示的实际效果；Linux 打包。

## 已知问题

- B 站和网易云用的都是非官方接口，会改版，也可能触发风控。出错时会给出中文提示，不会崩。
- B 站短信和密码登录依赖 geetest 的组件，加载失败时只能改用二维码。
- 部分 Linux 发行版的 WebView 没有 H.264 解码，B 站视频可能放不了。
- 局域网同步是明文 JSON，没有认证，只适合可信的内网。
- 翻页钟数字用的是随包字体，其他文字走系统字体。

## 授权

Fredoka 字体按 SIL OFL 1.1 使用。代码目前没有声明开源协议。

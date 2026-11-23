# Moka

> 极致轻量、100% 纯 Rust 原生的 Windows 全栈本地开发环境管理器（MIT 开源免费）。

Moka 旨在替代笨重的 Docker Desktop 与老旧集成环境面板：**零 Electron / 零 WebView / 零 Node.js**，采用暖色极简桌面美学（支持浅色 / 深色主题与 Windows 11 原生 DWM 圆角），所有运行时、数据库、配置与数据目录完全绿色隔离，零污染系统 PATH。

## 核心特性

- **100% 纯 Rust 原生桌面**：基于 `winit` + `eframe/egui` 硬件加速自绘，冷启动极快，最小化至系统托盘待机近 0% CPU。
- **20 款全栈 Windows 原生组件（一键安装 + 国内镜像自动容错）**：
  - **语言运行时 & 构建工具**（NTFS Junction 免 UAC 秒切版本）：Node.js (22 / 20 LTS)、Go (1.24)、Python (3.12)、OpenJDK (21 / 17 / 8 LTS)、Maven (3.9)、Gradle (8.12)
  - **数据库与缓存**：MySQL (8.4 / 8.0 / 5.7)、MariaDB (11.4 LTS)、PostgreSQL (17.4)、MongoDB (8.0)、Redis (7.2 Windows)
  - **Web / 微服务与测试工具**：Nginx (1.26)、Apache Tomcat (10.1)、Nacos (2.4)、Mailpit (1.24)
- **Nginx 多站点虚拟主机 + 内置本地 Root CA（绿锁 HTTPS）**：
  - 可视化管理任意多本地域名（如 `api.local`、`admin.local`），支持多条反向代理（CORS / WebSocket）、自动同步 Hosts 与 `nginx -t` + `nginx -s reload` 热重载；
  - 纯 Rust（`rcgen`）内置 `Moka Local Dev CA` 根证书，支持免 UAC 一键信任至当前用户受信任根存储，Chrome / Edge 本地 HTTPS 直接绿锁。
- **8 大服务可视化配置 + 高级源码格式化与语法校验**：
  - 覆盖 MySQL/MariaDB、Redis、Nginx、Nacos、PostgreSQL、Tomcat、MongoDB、Mailpit 全部 8 类可视化配置表单，支持与「高级源码」双向同步与保存前自动 `.bak` 备份；
  - 高级源码内置按文件类型区分的语法校验与一键美化格式化（Nginx / INI / Redis / Properties / PostgreSQL / XML / YAML / ENV），Nginx 额外执行 `nginx -t` 语义校验。
- **服务监控、快捷场景组与数据库快照**：
  - 实时采集运行中服务的内存占用与运行时长；支持自定义一键启动场景组、系统托盘悬浮面板 / 原生菜单即时切换；
  - 支持 MySQL / MariaDB / PostgreSQL 一键流式导出 `.sql` 快照与一键回滚恢复，以及 MySQL `root` 密码一键修改与连接串同步。
- **实用工具箱**：
  - **国内开发镜像加速中心**：一键切换 `npm`（npmmirror）、`pip`（清华 TUNA）、`Go GOPROXY`（goproxy.cn）、`Maven`（阿里云）国内源或恢复官方源；
  - **TCP 监听端口扫描与查杀**（虚拟滚动零卡顿）+ **Windows Hosts 可视化编辑**（自动 UAC 提权写入）。

## 从源码构建（Windows 10 / 11）

需要安装 [Rust 工具链](https://rustup.rs/) 与 Visual Studio C++ Build Tools (MSVC)：

```powershell
git clone https://github.com/xuanjinya168/moka.git
cd moka
cargo test --workspace
cargo build --release
```

编译产物位于 `target\release\moka.exe`，直接双击即可运行。

## 目录架构

```text
├── crates/
│   ├── moka-core/           # Manifest 安全校验、多镜像容错流式下载器、防 Zip Slip 原子解压引擎
│   ├── moka-process/        # Windows 无黑框进程管理、并行优雅停机、流式数据库备份恢复、轮转日志
│   ├── moka-config/         # 8 类可视化配置解析回写、多格式语法校验与格式化、本地 Root CA 签发、镜像加速
│   ├── moka-sys/            # TCP 端口扫描、NTFS Junction、Win11 DWM 圆角、托盘窗口管理、Hosts 与 CA 存储
│   └── moka-gui/            # 纯 Rust 原生桌面界面（main / pages / modals / config_form / widgets / theme / tray / storage）
└── manifests/
    └── catalog.json         # 内置 20 款全栈软件源清单
```

## 开源协议

本项目基于 [MIT License](LICENSE) 开源，永久免费，欢迎 Star、Issue 与 PR。

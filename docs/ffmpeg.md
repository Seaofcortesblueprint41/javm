完全可行。通过精简 FFmpeg 的编译配置，我们可以将其打造成一个**专职的“视频抽帧器”**。这种方案非常契合现代桌面端应用的打包需求，特别是对于使用 Tauri 和 Vue 这类技术栈构建的本地视频管理工具，一个极小的本地依赖组件能让最终的安装包体积大幅瘦身，同时避开繁杂的 C/C++ 运行库依赖。

### 一、 可行性与核心编译策略

FFmpeg 的架构是高度模块化的，支持在编译时通过 `./configure` 极其细粒度地裁剪功能。为了满足“支持所有视频格式”且“只抽帧保存图片”，我们的核心裁剪策略如下：

1. **保留解析与解码（Demuxers & Decoders）：** 为了支持所有视频格式（MP4, MKV, AVI, RMVB, 甚至各种小众/古老格式），我们必须保留绝大多数的 Demuxer（解封装器）和 Decoder（解码器）。
2. **砍掉所有无关组件：**
* **禁用网络模块：** 不需要 HTTP, RTMP, HLS 等流媒体协议，纯本地读取（`--disable-network`）。
* **禁用绝大多数编码器（Encoders）和封装器（Muxers）：** 这是**减体积的核心**。我们不需要把视频重新编码成 H.264 或 HEVC，只需要保留 JPEG、PNG 和 WebP 的图片编码器（`--enable-encoder=mjpeg,png,webp`）以及对应的封装器（`--enable-muxer=image2`）。
* **禁用硬件加速（HWAccels）：** 提取单帧通常是瞬间完成的，CPU 完全足够。禁用硬件加速能省去对各个系统底层图形库的依赖，极大增强跨平台稳定性。
* **禁用音频子系统：** 抽帧不需要处理声音，可以尝试禁用所有音频相关过滤器。


3. **保留关键滤镜（Filters）：** 仅保留缩放 (`scale`)、选帧 (`select`) 和缩略图 (`thumbnail`) 滤镜。

### 二、 各平台预估二进制体积

标准静态编译的 FFmpeg 体积通常在 70MB - 100MB 之间。经过上述极限裁剪后，我们只保留了解码器和图片输出能力：

| 操作系统 | 架构 | 预估单文件体积 | 备注说明 |
| --- | --- | --- | --- |
| **Windows** | x86_64 (`.exe`) | **16MB - 22MB** | 完全独立运行，无需安装任何额外的系统运行库。 |
| **macOS** | Universal (Intel + Apple Silicon) | **20MB - 28MB** | 由于包含了 x86 和 arm64 双架构代码，体积稍大，但方便分发。 |
| **Linux** | x86_64 | **15MB - 20MB** | 采用完全静态链接（musl libc），确保在所有 Linux 发行版上即插即用。 |

这个体积对于嵌入到桌面客户端中作为底层核心引擎来说，是非常理想的。

### 三、 GitHub Actions 跨平台编译脚本

下面是一个完整的 `ffmpeg-build.yml` 工作流脚本。为了保证跨平台编译的稳定性（尤其是 Windows 下直接用 MSVC 编译 FFmpeg 非常痛苦），这里采用了业界成熟的跨平台静态编译工具链 `ttpo/ffmpeg-build-script` 或者直接利用 MSYS2 和 macOS 的原生环境。

为了脚本的直观和可控，这里提供一套基于原生环境但注入了极限裁剪参数的 Actions 脚本：

```yaml
name: Build Minimal FFmpeg

on:
  push:
    tags:
      - 'v*.*.*' # 打标签时触发编译
  workflow_dispatch: # 支持手动触发

jobs:
  build-ffmpeg:
    name: Build FFmpeg on ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
          - os: ubuntu-latest
            artifact_name: ffmpeg-linux-x64
            exec_name: ffmpeg
          - os: macos-latest
            artifact_name: ffmpeg-macos-universal
            exec_name: ffmpeg
          - os: windows-latest
            artifact_name: ffmpeg-windows-x64.exe
            exec_name: ffmpeg.exe

    steps:
      - name: Checkout Code
        uses: actions/checkout@v4

      - name: Setup Windows Environment (MSYS2)
        if: runner.os == 'Windows'
        uses: msys2/setup-msys2@v2
        with:
          msystem: MINGW64
          update: true
          install: >-
            base-devel
            mingw-w64-x86_64-toolchain
            mingw-w64-x86_64-nasm
            mingw-w64-x86_64-yasm
            mingw-w64-x86_64-pkg-config

      - name: Setup Linux/macOS Dependencies
        if: runner.os != 'Windows'
        run: |
          if [ "$RUNNER_OS" == "Linux" ]; then
            sudo apt-get update
            sudo apt-get install -y build-essential yasm nasm pkg-config
          elif [ "$RUNNER_OS" == "macOS" ]; then
            brew install yasm nasm pkg-config
          fi

      - name: Download FFmpeg Source
        run: |
          curl -O https://ffmpeg.org/releases/ffmpeg-6.1.1.tar.bz2
          tar xjvf ffmpeg-6.1.1.tar.bz2
          mv ffmpeg-6.1.1 ffmpeg-src

      - name: Configure and Compile (Linux / macOS)
        if: runner.os != 'Windows'
        run: |
          cd ffmpeg-src
          ./configure \
            --disable-shared --enable-static \
            --disable-ffplay --disable-ffprobe --disable-doc \
            --disable-network \
            --disable-encoders --disable-muxers \
            --enable-encoder=mjpeg,png,webp \
            --enable-muxer=image2,image2pipe \
            --disable-hwaccels \
            --disable-vdpau --disable-videotoolbox --disable-vaapi \
            --enable-small \
            --extra-cflags="-Os"
          make -j$(nproc || sysctl -n hw.ncpu)
          mv ffmpeg ../${{ matrix.exec_name }}

      - name: Configure and Compile (Windows)
        if: runner.os == 'Windows'
        shell: msys2 {0}
        run: |
          cd ffmpeg-src
          ./configure \
            --disable-shared --enable-static \
            --disable-ffplay --disable-ffprobe --disable-doc \
            --disable-network \
            --disable-encoders --disable-muxers \
            --enable-encoder=mjpeg,png,webp \
            --enable-muxer=image2,image2pipe \
            --disable-hwaccels \
            --disable-d3d11va --disable-dxva2 \
            --enable-small \
            --extra-cflags="-Os" \
            --extra-ldflags="-static"
          make -j$(nproc)
          mv ffmpeg.exe ../${{ matrix.exec_name }}

      - name: Upload Artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact_name }}
          path: ${{ matrix.exec_name }}

```

### 四、 关键配置参数解析

* `--enable-small` 和 `--extra-cflags="-Os"`：告诉编译器针对**体积**而不是极致的运行速度进行优化，这对于不需要实时转码、只需要单帧抽取的工具来说非常划算。
* `--disable-encoders` / `--disable-muxers`：一刀切关闭所有输出能力，然后用 `--enable-encoder=mjpeg,png` 精准“开白名单”，这样就彻底切断了庞大的 x264/x265 等编码库的引入。
* `--extra-ldflags="-static"` (Windows)：强制静态链接 C 运行库，确保编译出来的 `.exe` 放到任何一台干净的 Windows 电脑上都不会报缺少 `.dll` 的错误。

有了这个精简版的二进制文件后，你在 Rust 后端中通过 `std::process::Command` 或者 `ffmpeg-sidecar` 调用它，就能获得全格式支持且极度轻量的抽帧体验。

需要我为你演示在 Rust 中调用这个精简版 FFmpeg 提取特定时间点帧并转为 Base64 或直接保存的最佳实践代码吗？
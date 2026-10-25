# Video geometry fixture

`dimensions-change.mp4` 是本仓生成的纯色测试片段，前 12 帧为红色
`32x32`，后 12 帧为蓝色 `64x32`，速率 `12 fps`，没有音频。
它用于验证播放期间拒绝尺寸变化，不包含外部素材。

可用 FFmpeg 重建：

```sh
ffmpeg -f lavfi -i color=c=red:s=32x32:r=12:d=1 -an -c:v libx264 -preset ultrafast -bf 0 -g 12 -f mpegts small.ts
ffmpeg -f lavfi -i color=c=blue:s=64x32:r=12:d=1 -an -c:v libx264 -preset ultrafast -bf 0 -g 12 -f mpegts wide.ts
cat small.ts wide.ts > change.ts
ffmpeg -i change.ts -c copy dimensions-change.mp4
rm small.ts wide.ts change.ts
```

## A/V clock fixtures (CC0)

`av-clock.mp4`、`silent.mp4` 和 `silent.webm` 均由以下命令生成，不含外部素材；
这三个测试文件按 CC0 1.0 提供。`av-clock.mp4` 包含 3 秒测试图案与 1.8 秒 440 Hz 正弦波，
供 WMF/Kira 音画时钟及 AVAssetReader seek/cancel 回归使用；两个静音文件用于音轨检测。

These three generated files are dedicated to the public domain under CC0 1.0.
They contain synthetic patterns/tones, with no third-party media.

```sh
ffmpeg -f lavfi -i testsrc2=size=64x64:rate=25:duration=3 -f lavfi -i sine=frequency=440:sample_rate=48000:duration=1.8 -c:v libx264 -pix_fmt yuv420p -bf 0 -g 25 -c:a aac -ac 2 -b:a 32k -movflags +faststart av-clock.mp4
ffmpeg -f lavfi -i color=black:size=16x16:rate=25:duration=0.08 -an -c:v libx264 -bf 0 -movflags +faststart silent.mp4
ffmpeg -f lavfi -i color=black:size=16x16:rate=25:duration=0.08 -an -c:v libvpx-vp9 silent.webm
```

The A/V sample uses 64×64 pixels because the native [WMF H.264 decoder](https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder) requires at least 48×48. The silent files are metadata-only fixtures.

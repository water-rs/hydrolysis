# Hydrolysis

> [!IMPORTANT]
> Hydrolysis now lives in [water-rs/waterui](https://github.com/water-rs/waterui) at [`backends/hydrolysis`](https://github.com/water-rs/waterui/tree/dev/backends/hydrolysis). This repository is archived; open issues and pull requests there.

Hydrolysis is WaterUI's self-drawn backend. It renders the retained view tree through Cherenkov — the GPU backend on phones, tablets and desktops, and the CPU backend at the microcontroller design point, selected per target at build time — and owns its desktop, web, accessibility, input, testing, examples, and release matrix independently from the WaterUI facade.

There is no runtime fallback between the two backends, and Hydrolysis does not bundle a widget theme. Applications select a compatible theme separately; the Material 3 implementation is maintained in `water-rs/hydrolysis-m3`.

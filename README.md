# Hydrolysis

Hydrolysis is WaterUI's GPU-required self-drawn backend. It renders the complete retained view tree through Vello on wgpu and owns its desktop, web, accessibility, input, testing, examples, and release matrix independently from the WaterUI facade.

Hydrolysis intentionally has no CPU fallback and does not bundle a widget theme. Applications select a compatible theme separately; the Material 3 implementation is maintained in `water-rs/hydrolysis-m3`.

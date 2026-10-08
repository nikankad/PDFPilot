# Project guidance

- Keep communication brief and use no emojis.
- PDFPilot is an independent application. Acrobat is a feature and visual reference; full parity is unfinished. Before changing feature coverage or claiming support, consult and update `FEATURES.md`. A menu entry alone does not make a feature complete.
- On macOS, application menus belong in the system menu bar. For menu changes, verify clicks, shortcuts, enabled states, and unsaved-document prompts in the running app; Rust tests cannot establish native UI behavior.
- Before changing reviews, sharing, or signature requests, read `SERVICE.md`. The independent PDFPilot service defaults to loopback. Public hosting requires explicit deployment work; typed signatures and certificate signatures have different guarantees.
- For test setup or packaging, follow the verification and engine requirements in `README.md`. Missing external engines or sandbox restrictions are environment failures, not grounds to weaken assertions.
- Use generated documents and certificates for verification. Preserve user documents and unsaved work; keep private service storage and signing keys out of commits.

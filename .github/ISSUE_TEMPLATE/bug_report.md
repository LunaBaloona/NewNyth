---
name: Bug report
about: Something isn't working as expected
title: '[BUG] '
labels: bug
assignees: ''
---

## Description
A clear description of what the bug is.

## Steps to reproduce
1.
2.
3.

## Expected behaviour
What you expected to happen.

## Actual behaviour
What actually happened.

## System information

- **NewNyth version:** (e.g. 0.7.1) — run `newnyth --version` or check the GUI sidebar
- **Mouse model:** Roccat Nyth Black / Roccat Nyth White
- **Firmware version:** (shown in the GUI left-sidebar, e.g. 1.38)
- **OS Distribution:** (e.g. Arch Linux)
- **Kernel:** (output of `uname -r`)
- **Desktop environment:** (e.g. KDE Plasma 6.x on Wayland / X11)
- **Rust version** (if building from source): (output of `rustc --version`)

## Daemon journal output

Please paste the last ~50 lines from the daemon log. Reproduce the bug, then:

```bash
journalctl --user -u nythd -n 100 --no-pager
```

<details>
<summary>Click to expand journal output</summary>

```
(paste output here)
```

</details>

## Additional context

Anything else that might help? Such as screenshots, config file contents
(`~/.config/newnyth/*.json`), other devices connected, etc.

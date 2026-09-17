# Upstream Research Pins

| Project | Revision | Role |
| --- | --- | --- |
| Oxide | `29cd89882465d6ebfe00af2ada6f89951581c580` | Selective audited runtime/security donor |
| gpui-component | `fb26e617da3add2ce2ac92a2ccc1a64bc8343135` | Native component foundation and synchronized Studio fork |
| gpui-pre | `0.3.5` (Zed snapshot `d89e9c2`) | Shipping GPUI distribution, Wayland-only |
| adabraka-ui | `e158684b23d9cb043fed3989ca252212046dabca` | Animation/component design reference |
| gpui-nav | `fecccf8c0d641efc75152fa206bbb941fa990c70` | Stack navigation reference |
| gpui-router | `b8b4228d9a1cb2bb108432241bcb5d8e6784a035` | Nested route-tree reference |

Pins are evidence for the initial architecture review. Shipping dependency revisions remain locked by `Cargo.lock` and may move only through an explicit upgrade review.

#import "/lib/lib.typ": *;
#let title = [文档];
#show: template;

本站（就是你现在看的这个）由 Typst 编译成 HTML。Rust 侧的驱动器在 `docs/` 下，负责拼出入口文件、调 Typst、处理路径。装工具链、跑编译、配编辑器这些*动手*的事见#link("/dev/docs/index.html")[文档（开发栏）]，这一页只讲它*怎么搭起来的*。

= 两层：生成入口 + 编译

Typst 的 `bundle` 特性要求有一个 `main.typ` 把所有页面和资源都 `include` 进来。这个文件不手写 —— Rust 驱动器扫描 `docs/typst/`，生成一份列全路径的 `main.typ`，再调 `typst compile`（带 `html` + `bundle` 特性）产出 `target/debug/docs/`。

所以「加一页」不需要改任何清单：把 `.typ` 放进目录，下一次构建就会被扫到。

= 增量：watch

`typst watch` 能在*已存在*的文件变化时增量重编，但它不知道*新增/删除*的文件。驱动器补上这一环 —— 它监听目录的增删，一旦有变化就重新生成 `main.typ`，再让 Typst 接着编。这就是#link("/dev/docs/index.html")[开发栏]那条 `--bin watch`。

= 路径：base path 怎么进去

站点发布在子路径下（`/rust2genshin/`），所以所有 `href` / `src` 都得带上前缀。前缀从环境变量 `BASE_PATH` 进来，转成 `typst sys.inputs` 传给编译器，在 Typst 侧存进 `contents.base_path`，生成 `href` / `src` 属性前手动加上。

页面里写的是站内绝对路径（`#link("/user/async.html")[...]`）。`template` 把这些以 `/` 开头的 `link` 认出来、补上 `base_path`、渲染成 `<a>` —— 所以正文里不用关心自己挂在哪一层。

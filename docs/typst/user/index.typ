#import "/lib/lib.typ": *;
#show: template;

#let title = [使用];
#let order = 0;

#set raw(lang: "rust");

这一栏讲 *怎么用 `rust2genshin` 写节点图* —— 你写的 Rust 会被编译成什么、有哪些写法、哪里会踩坑。

想搞懂「为什么编译出来是这样」，去#link("/arch/index.html")[架构]栏；想改 `rust2genshin` 本身，去#link("/dev/index.html")[开发]栏。

= 先读这三页

#list(
    link("/user/project.html", [项目定位]) + [ —— 它是什么、编译出什么],
    link("/user/getting-started.html", [上手]) + [ —— 装好工具链，跑通第一个 `.gia`],
    link("/user/handles.html", [句柄类型]) + [ —— `Guid` / `Faction` / `Config` / `Prefab`],
)

= 按目的找

*完全没用过节点图* —— 从「项目定位」开始，按上面的顺序读。

*会 Rust，想直接写* —— 直接看「句柄类型」，然后在 `demo/src/lib.rs` 里试。

*编译过了但结果不对* —— 看「兼容性与限制」和「不被支持的特性」，这两页是踩坑点集合。

= 全部页面

写法：

#list(
    link("/user/export.html", [导出函数]) + [ —— 哪些函数会成为复合节点，主图是什么],
    link("/user/events.html", [事件]) + [ —— `#[event_listener]` 和内置事件清单],
    link("/user/async.html", [异步]) + [ —— `#[asynchronous]` / `#[async_loop]` / `sleep!`，代替递归],
    link("/user/native.html", [原生调用与枚举]) + [ —— `lib/` 里那些没有函数体的函数，和 `#[native_enum(N)]`],
)

行为：

#list(
    link("/user/compatibility.html", [兼容性与限制]) + [ —— 能编译但语义和普通 Rust 不同的地方],
    link("/user/unsupported.html", [不被支持的特性]) + [ —— 编译不过的写法，以及替代方案],
    link("/user/limitations.html", [待办]) + [ —— 还没做的功能],
)

查 API：

#list(
    link("/user/capabilities.html", [能力速览]) + [ —— 有哪些分组、各自典型入口，不列全量签名],
)

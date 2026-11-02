#import "/lib/lib.typ": *;
#show: template;

#let title = [使用];
#let order = 0;
#let tag = <tag-user>;

#set raw(lang: "rust");

欲使用Rust编写节点图，您必须阅读此文档

本项目提供*在线体验*功能，前往#link(<tag-playground>)[*摆弄*]

其中可以加载*示例*，*编译*并立即*预览*结果

如果你已经会用*Rust*，可直接看#link(<for-rustaceans>)[Rust用户指南]

= 目标

可以选择*超限模式*或*经典模式*

= 产物

目前仅支持.gia，作为*资产包*导入游戏

其它格式尽情期待

= 页面

#list(
    link("/user/project.html", [项目定位]) + [ —— 它是什么],
    link("/user/tutorial/index.html", [入门]) + [ —— 跑通第一个`.gia`],
    link("/user/handles.html", [句柄类型]) + [ —— `Guid` / `Faction` / `Config` / `Prefab`],
    link("/user/export.html", [导出函数]) + [ —— 什么会成为复合节点],
    link("/user/events.html", [事件]),
    link("/user/async.html", [异步]) + [ —— 代替递归],
    link("/user/native.html", [原生调用与枚举]),
)

#list(
    link("/user/borrow.html", [借用]) + [ —— `&T` / `&mut T` / `PartialMut<T>`],
    link("/user/heap.html", [堆分配]) + [ —— `Box<T>`],
    link("/user/list.html", [列表]),
)

#list(
    link("/user/compatibility.html", [兼容性与限制]) + [ —— 能编译，但语义不同],
    link("/user/unsupported.html", [不被支持的特性]) + [ —— 编译不过，附替代],
    link("/user/limitations.html", [待办]),
    link("/user/capabilities.html", [能力速览]) + [ —— 有哪些API],
)

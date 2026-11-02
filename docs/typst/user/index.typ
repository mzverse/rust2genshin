#import "/lib/lib.typ": *;
#show: template;

#let title = [使用];
#let order = 0;
#let tag = <tag-user>;

#set raw(lang: "rust");

欲使用Rust编写节点图，您必须阅读此文档。

本项目提供*在线体验*功能，前往#link(<tag-playground>)[*摆弄*] —— 可以加载*示例*、*编译*并立即*预览*结果。

已经会用*Rust*的，直接看#link(<for-rustaceans>)[Rust用户指南]。

= 从这里开始

#list(
    link("/user/tutorial/index.html", [入门]) + [ —— 装好工具链，跑通第一个 `.gia`],
    link("/user/handles.html", [句柄类型]) + [ —— `Guid` / `Faction` / `Config` / `Prefab`，到处都是],
    link("/user/capabilities.html", [能力速览]) + [ —— 有哪些 API 可调],
)

= 目标与产物

可以选择*超限模式*或*经典模式*

目前仅支持 `.gia`，作为*资产包*导入游戏

其它格式尽情期待

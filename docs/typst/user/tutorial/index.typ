#import "/lib/lib.typ": *;
#show: template;

#let title = [教程];

跟随本章您将一步步了解如何使用该项目

#list(
    link("/user/tutorial/0.deploy.html", [部署]) + [ —— 装好工具链，跑通第一个 `.gia`],
    link("/user/tutorial/1.start.html", [开始]) + [ —— 写第一个复合节点，加参数和返回值],
    link("/user/tutorial/2.variable.html", [变量]) + [ —— 类型、可变性、作用域与遮蔽],
)

已经会 Rust、只想看差异的，直接看#link("/user/for-rustaceans.html")[Rust 用户指南]。

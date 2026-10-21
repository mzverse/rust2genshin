#import "/lib/lib.typ": *;
#show: template;

#let title = [复合节点];

#import "/lib/graph.typ";
#import "/lib/card.typ": *;

#set raw(lang: "rust");

让我们先来写一个*复合节点*

在Rust中，一个*函数*对应一个复合节点

使用`fn`关键字创建这个函数，就叫"my_composite"好了：

```rust
fn my_composite() {
    log("Hello");
    log("Composite");
}
```

#graph.node({
    import graph: *;
    [my_composite]
    ctrl_in()
    ctrl_out()
});

其中我们使用了`log()`来*打印日志*，括号里的即为日志内容

现在还不能导入到游戏，我们必须标上`#[unsafe(no_mangle)]`使其成为*主要资产*

```rust
#[unsafe(no_mangle)]
fn my_composite() {
    log("Hello");
    log("Composite");
}
```

#card_info[
    只有*主要资产*和被主要资产（间接）*使用*的资产才会被*导入*

    否则游戏认为它们是*没用的*
]

= 参数

我们的复合节点被使用时，外部可以传递*参数*给我们

```rust
#[unsafe(no_mangle)]
fn my_composite(a: String) {
    log("Hello");
    log(a);
}
```

#graph.node({
    import graph: *;
    [my_composite]
    ctrl_in()
    ctrl_out()
    data_in("String")[a]
});

我们函数的括号内写的`a: String`则表示一个名为`a`的参数，类型为`String`即字符串（文本）

然后我们使用`log(a)`将其打印到了日志中

多个参数使用半角逗号分隔

```rust
#[unsafe(no_mangle)]
fn my_composite(a: String, b: i32) {
    log("Hello");
    log(a);
    log(b);
}
```

#graph.node({
    import graph: *;
    [my_composite]
    ctrl_in()
    ctrl_out()
    data_in("String")[a]
    data_in("Int")[b]
});

这里的`i32`表示整数类型

= 返回值

我们也可以将数据传递给复合节点的使用者

```rust
#[unsafe(no_mangle)]
fn my_composite(a: i32) -> i32 {
    log("Hello");
    return a * 2 + 1;
}
```

#graph.node({
    import graph: *;
    [my_composite]
    ctrl_in()
    ctrl_out()
    data_in("Int")[a]
    data_out("Int")[这里是返回值]
});

这里使用`-> i32`表示返回值的类型是整数

然后计算`a * 2 + 1`，使用`return`将得到的数返回

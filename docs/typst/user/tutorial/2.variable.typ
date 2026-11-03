#import "/lib/lib.typ": *;
#show: template;

#let title = [变量];

#set raw(lang: "rust");

#import "/lib/graph.typ";
#import "/lib/card.typ": *;

使用`let`声明*变量*

`=`左边是*变量名*，右边是它的*初始值*：

```rust
fn show() {
    let phase = "第一阶段";
    log(phase);
    log(phase);
}
```

#graph.graph({
    import graph: *;
    node("log1", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("log2", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    ctrl(("log1", 0), ("log2", 0))
    data_default(("log1", 0))[`"第一阶段"`]
    data_default(("log2", 0))[`"第一阶段"`]
    ctrl_in_extern(("log1", 0))
    ctrl_out_extern(("log2", 0))
})

= 可变性

若变量需被改变，则应声明为`mut`：

```rust
fn show() {
    let mut phase = "第一阶段"; // 加上 mut
    log(phase);
    phase = "第二阶段"; // 在此处修改
    log(phase);
}
```

#graph.graph({
    import graph: *;
    node("local", comment: "phase", {
        [获取局部变量]
        data_out("LocalRef")[变量引用]
        data_out("String")[值]
    })
    node("set1", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("log1", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("set2", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("log2", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    data_default(("set1", 1))[`"第一阶段"`]
    ctrl(("set1", 0), ("log1", 0))
    ctrl(("log1", 0), ("set2", 0))
    data_default(("set2", 1))[`"第二阶段"`]
    ctrl(("set2", 0), ("log2", 0))
    ctrl_in_extern(("set1", 0))
    ctrl_out_extern(("log2", 0))
    data(("local", 0), ("set1", 0))
    data(("local", 0), ("set2", 0))
    data(("local", 1), ("log1", 0))
    data(("local", 1), ("log2", 0))
})

对变量的*修改*称为*赋值*

每个变量对应一个*获取局部变量*节点

为变量*赋值*则对应*设置局部变量*节点

#card_tip[
    不可变的为什么还叫*“变量”*？

    因为它的值可以是*运行时*才得到的，而不一定是*常量*

    又或是该代码被执行了多次，那它其实也被赋值了多次
]

= 类型

变量的类型是明确的，可以手动*标注类型*，写法是名字后面跟 `: 类型`：

```rust
let speed: f32 = 1.5;
let alive: bool = true;
let name: String = "旅行者";
```

#card_tip[
    类型通常能*自动推导*，无需标注
]

#table(
    columns: (auto, 1fr, auto),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [*类型*], [*说明*], [*例*],
    [`i32`], [整数], [`let hp: i32 = 100;`],
    [`f32`], [浮点], [`let speed: f32 = 1.5;`],
    [`bool`], [布尔，`true` / `false`], [`let alive: bool = true;`],
    [`String`], [字符串], [`let name: String = "旅行者";`],
)

= 参数

参数本质上也是变量，由*调用者*提供初始值

#card_attention[
    *参数必须标注类型*
]

```rust
fn show(phase: String) {
    log(phase);
    log(phase);
}
```

#graph.graph({
    import graph: *;
    node("set", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("local", comment: "phase", {
        [获取局部变量]
        data_out("LocalRef")[变量引用]
        data_out("String")[值]
    })
    node("log1", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("log2", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    ctrl(("set", 0), ("log1", 0))
    ctrl(("log1", 0), ("log2", 0))
    ctrl_in_extern(("set", 0))
    ctrl_out_extern(("log2", 0))
    data(("local", 0), ("set", 0))
    data(("local", 1), ("log1", 0))
    data(("local", 1), ("log2", 0))
    data_in_extern(("set", 1))
})

= 返回值

函数也可以把结果交回给*调用者*，写法是参数列表后面加 `-> 类型`：

```rust
fn bonus(level: i32) -> i32 {
    let extra = level * 10;
    return extra + 1;
}
```

`return` 后面的值是*返回*出去的东西，它的类型必须和 `->` 后面写的一致。

#graph.graph({
    import graph: *;
    node("mul", {
        [乘法]
        data_in("Int")[A]
        data_in("Int")[B]
        data_out("Int")[结果]
    })
    node("add", {
        [加法]
        data_in("Int")[A]
        data_in("Int")[B]
        data_out("Int")[结果]
    })
    data_default(("mul", 1))[10]
    data_default(("add", 1))[1]
    data(("mul", 0), ("add", 0))
    data_in_extern(("mul", 0))
    data_out_extern(("add", 0))
})

看*入口*那两根数据引脚：一根是参数 `level`（从外面进来），一根是*返回值*（交给外面）。

返回值引脚直接接在算它的那个节点上 —— 它不会从头再算一遍，就是那个值的出口。

#card_attention[
    有些函数*只有*返回值，不碰任何外部状态：不打印、不读写实体，只做计算。

    这时图上连*入口*节点都没有，整张图就是一个算式 —— 参数引脚直接接进第一个运算节点。
]

= 作用域

从变量声明的地方开始，直到它所在的*块*（`{}`）结束，称为其*作用域*

在作用域内你才能*访问*到该变量

```rust
fn show(base: String) { // 函数是一个块
    { // 你也可以再写一个小的块
        let hint = "准备"; // 作用域在小块内
        log(hint); // 这里可以访问
    } // hint 的作用域结束，下面无法再访问
    log(base);
    log(base);
}
```

#graph.graph({
    import graph: *;
    node("set", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("local", comment: "base", {
        [获取局部变量]
        data_out("LocalRef")[变量引用]
        data_out("String")[值]
    })
    node("log1", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("log2", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("log3", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    ctrl(("set", 0), ("log1", 0))
    ctrl(("log1", 0), ("log2", 0))
    ctrl(("log2", 0), ("log3", 0))
    ctrl_in_extern(("set", 0))
    ctrl_out_extern(("log3", 0))
    data(("local", 0), ("set", 0))
    data(("local", 1), ("log2", 0))
    data(("local", 1), ("log3", 0))
    data_default(("log1", 0))[`"准备"`]
})

*访问*的意思就是用变量名表示这个变量，包括*读取*和*赋值*

任意一对花括号都是一个*块*，包含*函数*、*分支*和*循环*等中的

内层能访问外层的变量，反过来则不行——因为内层要么还没开始、要么已经结束

== 遮蔽

其实就是*名称相同*的变量，这是允许的

如果我在*多个同名变量*的作用域内，到底访问的是*哪个*呢？

答案是*最近*（最后）的那个

```rust
fn show(act1: String, act2: String) {
    let phase = act1;
    log(phase);
    let phase = act2; // 名称相同，但其实是不同的变量
    log(phase); // 访问到的是第二个
}
```

#graph.graph({
    import graph: *;
    node("set1", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("set2", {
        [设置局部变量]
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[变量]
        data_in("String")[值]
    })
    node("local1", comment: "phase ①", {
        [获取局部变量]
        data_out("LocalRef")[变量引用]
        data_out("String")[值]
    })
    node("local2", comment: "phase ②", {
        [获取局部变量]
        data_out("LocalRef")[变量引用]
        data_out("String")[值]
    })
    node("log1", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    node("log2", {
        [打印日志]
        ctrl_in()
        ctrl_out()
        data_in("String")[内容]
    })
    ctrl(("set1", 0), ("log1", 0))
    ctrl(("log1", 0), ("set2", 0))
    ctrl(("set2", 0), ("log2", 0))
    ctrl_in_extern(("set1", 0))
    ctrl_out_extern(("log2", 0))
    data(("local1", 0), ("set1", 0))
    data(("local2", 0), ("set2", 0))
    data(("local1", 1), ("log1", 0))
    data(("local2", 1), ("log2", 0))
    data_in_extern(("set1", 1))
    data_in_extern(("set2", 1))
})

两个 `phase` 是*两个不同的变量* —— 图上就是两个*获取局部变量*，两个*设置局部变量*，各管各的。后一个不会覆盖前一个，两者只是重名。

正因为互不相干，类型也可以不一样：

```rust
let hp = 100;
log(hp);
let hp = "很低";
log(hp);
```

后一个 `hp` 是字符串，和前一个整数 `hp` 毫无关系。换成 `hp = "很低"` 就编译不过 —— 那是给同一个变量赋一个类型不同的值。

这和*可变性*不是一回事：

#table(
    columns: (auto, auto, auto, auto),
    inset: 6pt,
    fill: (_, y) => if calc.odd(y) { luma(235) },
    [], [*写法*], [*几个变量*], [*能换类型*],
    [重新赋值], [`hp = 12`], [一个], [不能],
    [遮蔽], [`let hp = 12`], [两个], [能],
)

#card_tip[
    遮蔽常用来把一个值加工成新的同名值，比如 `let hp = hp - 30;`。

    不过右边那个 `hp` 得真的被读出来 —— 旧值只在这一处用到的话，编译器会直接展开，
    图上就看不到两个局部变量了。
]

= 优化说明

基本上你认为多余的变量都会被*自动消除*，无需担心

后续我们还会加入更多优化

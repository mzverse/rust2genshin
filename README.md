# 简介

**非官方**的 `rustc` codegen 后端:把目标 crate 的 Rust 代码编译成原神的节点图资产文件 `.gia`。

目前仅支持服务器节点图。

# Usage

## Demo

### 监听选项卡选中事件

```rust
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid); // 打印Guid
    event.entity.delete(); // 删除本实体
}
```

### 简单的求根公式

```rust
#[unsafe(no_mangle)] // 导出此函数为复合节点
pub fn solve(a: f32, b: f32, c: f32) -> f32 {
    (- b + delta(a, b, c).sqrt()) / (2. * a)
}

fn delta(a: f32, b: f32, c: f32) -> f32 {
    b * b - 4. * a * c
}
```

## Build

可参见github workflow

1. 确保安装了[rustup](https://rustup.rs/)、cargo和[protoc](https://github.com/protocolbuffers/protobuf/releases)
2. 克隆本项目
    ```shell
   git clone https://github.com/mzverse/rust2genshin
   ```
3. 将`demo`文件夹重命名，同时记得改项目根目录的`Cargo.toml`的`members`
4. 在重命名后的demo的`lib.rs`中编写自己的节点图（Rust代码）
5. 构建demo
    ```shell
    cargo run -p build-demo
    ```
    若改了`demo`的模块名，需同时修改`build-demo`的代码
6. 构建结果是`target/rust2genshin_demo.gia`

## Code

### 监听事件

在`pub fn`上添加属性`#[event_listener]`

且唯一参数是`event`，例如

```rust
#[event_listener]
pub fn on_tab_selected(event: TabSelectedEvent) {
    log(event.guid);
    event.entity.delete();
}
```

事件节点和调用会被加入到主图中

### 导出函数

将函数导出为复合节点图，声明为`#[unsafe(no_mangle)]`或`#[unsafe(export_name = "导出的名称")]`：
```rust
#[unsafe(no_mangle)]
pub fn my_composite() {
    // this fn will be exported as composite node
}
```

> [!IMPORTANT]
> 主图始终导出（如果存在），未导出或未被导出资产引用的资产无法被导入

### 句柄类型

`Guid` / `Faction` / `Config` / `Prefab` 是"指向游戏数据的引用"类型，**只能在编译期构造**。用对应的 `xxx!` 宏：

```rust
let g = guid!(42);         // 编译期生成 Guid(42)
let f = faction!(1);       // Faction
let c = config!(100);      // Config
let p = prefab!(7);        // Prefab
```

宏参数必须是 const 表达式 —— 字面量、`const` 绑定、算术常量都行。运行期变量会被编译器拒掉，因为 `Guid::new` 等标了 `#[rustc_comptime]`（不是 `const fn`），**不在 const 上下文调用直接编译失败**：

```rust
let id = compute_id();              // 运行期变量
let g = guid!(id);                  // 编译错：id 不是 const 表达式
let g = Guid::new(42);              // 编译错：comptime fn 只允许 const 上下文
let _ = some_guid.0;                // 编译错：字段私有
some_guid.0 = 99;                   // 编译错：字段私有
```

等价写法（宏只是把 `const { Xxx::new(x) }` 藏起来）：

```rust
const G: Guid = Guid::new(42);      // 顶层 const
let g = const { Guid::new(42) };    // 内联 const 块
```

# 兼容性

- 运算溢出

    为性能起见，`i32`的四则运算默认自动溢出，即原生的运算节点

- 整数除法

    - 除数为`0`时触发错误并得到`0`
    - `i32::MIN / -1`等于`0`

- 扁平化局部变量

    在局部变量（包括参数和返回值）中

    元组、闭包和事件在编译后会（递归）展开
    
    以支持多个输出（返回值）

- 内部可变性
    
    不支持`Cell`等内部可变性

    将来可能会支持`RcRefCell`和`RcUnsafeCell`

- 可变借用
    
    只能对局部变量本身取可变借用，而不能对成员

# Todo List

## 语言特性

- `enum`
- loops
- `async fn`(coroutine)
- 客户端节点图

## 类型

- `VarSnapshotRef`
- `Vec3`
- `Vec2`
- `List<T>` & `[T]`
- `Dict<K, V>`
- `Box<T>`
- unsigned int
- `i64`

## 原生节点

待完善

## 原生枚举

暂不支持`match`

待完善

## 事件

待完善

## 编译流程

正确的流程应为分别编译每个crate，再link得到最终.gia/.gil/.gis

但.gia本身不支持link，所以我们现在先偷懒直接编译目标crate了

后果就是无法获取依赖中的MIR

# 不被支持的特性

- `i8`, `i16`：请使用`i32`
- `#[repr(u32)]`等，请使用`#[repr(i32)]`
- 递归调用：请改写为循环，或改用`async fn`然后`await`
- trait object（`dyn`）：虚表开销过大，可能不予支持

# 关于节点图

## 数据流

- 对于有控制流的节点（如执行节点或事件，除了列表迭代循环）：节点本身储存所有出参，就像局部变量节点一样可以直接获取

- 对于无控制流的节点（如查询节点或运算节点，除了局部变量）：每次获取其出参时，其**重新获取**入参并计算结果

### 默认值

- 若入参未连接且未设置值，则使用该类型的默认值

- 若可执行节点未被执行过（包括之前的事件），也使用该类型的默认值

结构体的默认值在结构体定义中设置

### ‘获取局部变量’节点

此节点储存了一个值（MaybeUninit），可以用‘设置局部变量’修改

直接获取其值时，若未被设置，则以‘初始值’初始化

不例外地，**每次**访问此节点，它均会访问‘初始值’引脚（因为这是**入参**），**无论是否已初始化**

因此**不建议**将‘初始值’连接到其它节点

在每次事件开始时，局部变量均已确保是未初始化的（已验证）

### ‘有限循环’

不例外地，只储存‘当前循环值’，在进入时将其设为‘循环起始值’。‘循环终止值’每次都获取而**不储存**

### ‘列表迭代循环’
此节点不储存元素，也不储存列表长度，相当于
```cpp
for(int i = 0; i < list.size(); i++) {
    auto &element = list[i];
    // body
}
```

### 所有权

（节点图、局部）变量和（列表、结构体）成员、储存的出参始终持有数据而非引用，数据存进它们时自动**克隆**

其它地方均为引用（借用）

### 节点图变量修改事件

当’设置节点图变量‘且’触发事件‘为`true`时，其立即**克隆**旧数据和新数据，生成修改事件并添加进事件队列中

## 控制流

排除“退出循环”节点，控制流不允许有环路

反复执行的块必须被反复重新进入，例如使用原生的循环节点

### “退出循环”

本质上只是给所有目标循环打上终止标记，并不能直接影响控制流

可以理解为“下一轮跳出循环”

### 有限循环

包括上下界，使用`<=`。当上界为`i32::MAX`时无法正常结束，相当于
```cpp
for(int i = begin; i <= end; i++)
```

## 运算

- ‘模运算’节点实际上是**取余**而不是取模

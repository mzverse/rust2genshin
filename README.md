# Usage

## Demo

简单的求根公式：

```rust
#[unsafe(no_mangle)]
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
2. 安装Rust nightly和组件
    ```shell
   rustup +nightly component add rustc-dev rust-src llvm-tools-preview
    ```
3. 克隆本项目
    ```shell
   git clone https://github.com/mzverse/rust2genshin
   ```
4. 将`demo`文件夹重命名，同时记得改项目根目录的`Cargo.toml`的`members`
5. 在重命名后的demo的`lib.rs`中编写自己的节点图（Rust代码）
6. 构建demo
    ```shell
   cargo +nightly run -p build-demo
   ```
7. 构建结果是`target/rust2genshin_demo.gia`

## Code

### 监听事件

TODO

事件节点期望加入到主图中

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

# 兼容性

- 运算溢出

    为性能起见，`i32`的四则运算默认自动溢出，即原生的运算节点

- 整数除法
    
    `i32`的除法默认会生成一堆检查，请改用`divide`方法，对应**除法节点**

    注意`i32::MIN.divide(-1)`等于`0`

# Todo List

- cast
- `struct`
- events
- tuple
- loops
- `Box`
- `async fn`
- closure
- unsigned int
- `i64`
- client node graph

# 不被支持的特性

- `i8`, `i16`：请使用`i32`
- 递归调用：请改写为循环，或改用`async fn`然后`await`
- trait object（`dyn`）：虚表开销过大，可能不予支持

# 关于节点图

## 数据流

- 对于有控制流的节点（如执行节点或事件，除了列表迭代循环）：节点本身储存所有出参，就像局部变量节点一样可以直接获取

- 对于无控制流的节点（如查询节点或运算节点，除了局部变量）：每次获取其出参时，其**重新获取**入参并计算结果

### ‘获取局部变量’节点

此节点储存了一个值（MaybeUninit），可以用‘设置局部变量’修改

直接获取其值时，若未被设置，则以‘初始值’初始化

不例外地，**每次**访问此节点，它均会访问‘初始值’引脚（因为这是**入参**），**无论是否已初始化**

因此**不建议**将‘初始值’连接到其它节点

在每次事件开始时，局部变量均已确保是未初始化的（已验证）

### ‘有限循环’

不例外地，只储存·当前循环值·，在进入时将其设为‘循环起始值’。‘循环终止值’每次都获取而**不储存**

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
- 

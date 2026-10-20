#import "/lib/lib.typ": *;
#show: template;

#let title = [SetLocal];
#let tag = <tag-set-local>;

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        "SetLocal<T>"
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[setter]
        data_in("T")[value]
    })
}

将#link(<tag-local>)[Local]设为指定值

= 消除

当变量为本地节点时，对其setter进行可能的消除

详见```rust Optimizer::eliminate_unnecessary_local_setter```

== 约定

- setter代表可能对该变量造成影响的执行节点

    暂时只考虑必定重设该变量的情况

- `&mut`的值（`PartialMut`）被交给`Modify`（改字段）或函数调用时，那是在**通过引用写**它，
  不是读它 —— 那处usage不算执行点，不用沿控制流判断它读到的值是谁写的，
  直接改接到写入的值上

- 对于该变量的一处使用，executors代表所有触发该使用的执行节点

- 执行点读到的值，由它「之前最后一个写这个变量的setter」决定：

    沿执行点的控制输入往回走，遇到写这个变量的setter就停 —— 它之后的值就是它写的，
    它之前的setter都被它盖掉了；走到别的节点就合并各条控制输入路径的结果

    合并出多个setter说明有分支、两边写的值不一样，属于不明确；控制流成环同样不明确

    注意执行点自己如果是setter，它写下的值在读取之后，不算「之前的setter」

- 变量的usage可分为3种：

    - 完全取决于当前setter

    - 完全与当前setter无关的，比如它前面总是其它setter

    - 不明确，比如有分支

    当所有usages均明确且不超过阈值，进行消除

- 消除setter相当于把它写入的值挪到usage那里求值

    所以写入的值必须是「挪了也一样」的：pure的值挪到哪里都一样；不纯的值每次求值
    可能不同，挪了就变了

- opaque代表「说不清它求值时是什么值」的节点

    控制流回溯时碰到它，说明那个执行点读到的值来源不明，不明确，拒绝优化

== 外部

=== setter

局部变量的引用不会逃逸，可忽略

=== usage

视为`LinkTarget::Export`，照常处理

== 步骤

+ 找到所有setters

    `Local`的句柄（`values_out[0]`）被谁消费了，谁就是它的setter

+ 找出写入值的来源里的opaque节点

    写入的值不一定是pure的，挪到usage那里求值可能就变了，所以要先看它从哪来

    从setter的`value in[1]`的来源开始BFS（值流不会有环）：

    - 来自extern：跳过

    - 不是calc节点：这是opaque

    - 是`Local`节点：它所有的setter都是opaque；值是ADT（能拿到`PartialMut`）的话，
      这个值的使用也要一起看

    - 是其它不纯的节点：拒绝优化

    - 是pure节点：把它的所有`value in`入队，继续

+ 遍历每一处usage：

    + 找到它的所有executors

        `&mut`的值被交给`Modify`或函数调用时不算执行点（那是写），不用回溯控制流，
        但它的输入同样要改接到写入的值上 —— 否则这个local永远消不掉

    + 根据所有executors，得到该usage类型：

    若不明确就直接拒绝优化，若有关当前setter则记录

+ 消除该setter并将值连接到有关usages

    被写入的值不能依赖这个变量的旧值 —— 否则把usage改接到写入的值上会造环
    （`i += 1` 就是这种：写入值是 `i + 1`，而 `i` 正是usage）

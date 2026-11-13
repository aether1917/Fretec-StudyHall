//! 冒烟测试目标：让包拥有显式测试目标，从而让 build.rs 里的
//! `cargo:rustc-link-arg-tests`（Common Controls v6 清单）生效。
//! 真正的单元测试位于各模块内部。

#[test]
fn crate_links_and_loads() {
    assert_eq!(2 + 2, 4);
}

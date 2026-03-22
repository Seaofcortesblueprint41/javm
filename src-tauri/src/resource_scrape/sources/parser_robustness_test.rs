//! 属性测试：解析器健壮性——无效输入不崩溃
//!
//! **Validates: Requirements 8.5**
//!
//! 对所有 Source 解析器使用随机字符串调用 parse()，
//! 验证不 panic 且返回 None（因为随机字符串不是有效 HTML）。

use proptest::prelude::*;
use super::{
    Source,
    javbus::Javbus,
    javsb::JavSb,
    javmenu::Javmenu,
    javplace::JavPlace,
    javxx::JavXX,
    projectjav::ProjectJav,
    threexplanet::ThreeXPlanet,
};

/// 获取所有解析器实例
fn all_parsers() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(Javbus),
        Box::new(JavSb),
        Box::new(Javmenu),
        Box::new(JavPlace),
        Box::new(JavXX),
        Box::new(ProjectJav),
        Box::new(ThreeXPlanet),
    ]
}

proptest! {
    /// Property 3: 解析器健壮性
    ///
    /// 对于任意随机字符串（包括空字符串、随机 UTF-8、随机 ASCII），
    /// 对所有解析器调用 parse(html, code) 应返回 None 而非产生 panic。
    ///
    /// **Validates: Requirements 8.5**
    #[test]
    fn parser_does_not_panic_on_random_input(
        html in "\\PC{0,500}",
        code in "\\PC{0,50}",
    ) {
        for parser in all_parsers() {
            // 调用 parse 不应 panic，结果应为 None（随机字符串不是有效 HTML）
            let result = parser.parse(&html, &code);
            prop_assert!(
                result.is_none(),
                "解析器 {} 对随机输入返回了 Some，html 长度={}, code='{}'",
                parser.name(),
                html.len(),
                code
            );
        }
    }

    /// 空字符串输入不崩溃
    ///
    /// **Validates: Requirements 8.5**
    #[test]
    fn parser_does_not_panic_on_empty_input(
        code in "\\PC{0,50}",
    ) {
        for parser in all_parsers() {
            let result = parser.parse("", &code);
            prop_assert!(
                result.is_none(),
                "解析器 {} 对空 HTML 返回了 Some",
                parser.name()
            );
        }
    }
}

// ============================================================
// static/weight_converter.js —— 重量换算器（纯前端，M4 第 4 步）
// ============================================================
// 【教学说明】
// 训练中常要"片重 → 总重"换算。这是纯前端工具：
//   不碰后端、不查数据库，输入片重实时显示总重。
// 换算只是输入辅助，不是业务逻辑，所以不用 Rust 实现。
//
// 四种模式（与 exercises 表的 default_mode 一致）：
//   bar    杠铃：总重 = 杆重 + 2 × 片重（一侧一片）
//           杆重可选 0（倒蹲等无杆动作：片挂轴上，轴不称重）
//   support 自重：总重 = 体重 − 支撑量
//           （支撑器械标的是"帮你抵消多少体重"，如 90kg 体重
//             用 30kg 支撑做引体 → 实际负重 = 90 − 30 = 60kg）
//   std    器械：总重 = 片重（机器配重直接读数）
//   lb2kg  磅制：总重 = 片重 × 0.4536（1 磅 = 0.4536 kg）
//
// 用法（HTML 里）：
//   <select id="mode-select">…</select>
//   <input id="plate-input" type="number">   ← bar/std/lb2kg 是片重；support 是支撑量
//   <input id="bar-input" type="number">     ← 仅 bar 模式显示
//   <input id="body-input" type="number">    ← 仅 support 模式显示（体重，localStorage 记住）
//   <span id="result"></span>
//   <button id="fill-btn">填入重量</button>
//   页面把动作 bar_weight 渲染进 <body data-bar-weight="20">，
//   JS 从这里读初始杆重。
// ============================================================

// 【教学：换算函数 —— 纯函数式（无副作用）】
// 输入 (mode, plate, bar, body) → 输出总重。
// 纯函数 = 同样的输入永远同样的输出，不读外部状态。
// 这样方便单测、好推理。四个分支用 switch（JS 版 match）。
function convertWeight(mode, plate, bar, body)
{
    // 非数字输入 → 0（输入框清空时不算错）
    const plateKg = Number(plate) || 0;
    const barKg = Number(bar) || 0;
    const bodyKg = Number(body) || 0;
    switch (mode)
    {
        case 'bar':
            // 杠铃：两侧各一片 → 2 × 片重 + 杆重
            return barKg + 2 * plateKg;
        case 'support':
            // 【教学：support 是"抵消"不是"负重"】
            // 支撑器械标的是帮身体抵消的重量（counterweight）：
            //   90kg 体重 + 30kg 支撑做引体 → 实际负重 60kg
            // 体重小于支撑量时 clamp 到 0（不可能负负重）
            return Math.max(0, bodyKg - plateKg);
        case 'std':
            // 器械：配重读数就是总重
            return plateKg;
        case 'lb2kg':
            // 磅制：× 0.4536
            return plateKg * 0.4536;
        default:
            return 0;
    }
}

// 【教学：四舍五入到 0.5 —— 健身片重都是 0.5 的倍数】
// Math.round(x * 2) / 2：先放大 2 倍取整再缩回，得到 0.5 的倍数。
// 例：62.36 → 62.5；61.1 → 61（Math.round 四舍五入）
function roundToHalf(x)
{
    return Math.round(x * 2) / 2;
}

// 【教学：DOM 操作 —— 页面加载后绑定事件】
// 模块级 init 函数：找到页面的换算器元素，绑定输入事件。
// 用 data-* 属性从后端渲染的 HTML 读初始值（见 record_form 的注释）。
function initWeightConverter()
{
    // 【教学：Optional Chaining（?.）—— 找不到元素不报错】
    // 换算器只在记录页有，其他页面调用 initWeightConverter 时
    // getElementById 返回 null，?. 短路返回 undefined，不会崩。
    const modeSelect = document.getElementById('mode-select');
    if (!modeSelect)
    {
        return; // 页面没有换算器（如今日页）→ 什么都不做
    }

    const plateInput = document.getElementById('plate-input');
    const barRow = document.getElementById('bar-row');
    const barInput = document.getElementById('bar-input');
    const bodyRow = document.getElementById('body-row');
    const bodyInput = document.getElementById('body-input');
    const result = document.getElementById('result');
    const fillBtn = document.getElementById('fill-btn');
    const weightInput = document.getElementById('weight-input');

    // 从 <body data-bar-weight="20"> 读初始杆重（后端渲染进 HTML）
    const defaultBar = Number(document.body.dataset.barWeight) || 20;
    // 体重从 localStorage 读（记过一次后下次自动带出，不用每次填）
    const defaultBody = Number(localStorage.getItem('weight_converter_body')) || 70;

    // 【教学：updateResult —— 读输入 → 换算 → 写显示】
    const updateResult = () =>
    {
        // 当前模式
        const mode = modeSelect.value;
        // bar 模式显示杆重行；support 模式显示体重行；其他模式都隐藏
        barRow.style.display = mode === 'bar' ? '' : 'none';
        bodyRow.style.display = mode === 'support' ? '' : 'none';
        // 换算（片重/支撑量 + 杆重 + 体重）→ 总重
        const total = convertWeight(
            mode,
            plateInput.value,
            barInput.value || defaultBar,
            bodyInput.value || defaultBody,
        );
        // 显示（0.5 的倍数）
        result.textContent = roundToHalf(total) + ' kg';
    };

    // 【教学：事件监听 —— input 事件（每次输入都触发）】
    modeSelect.addEventListener('input', updateResult);
    plateInput.addEventListener('input', updateResult);
    barInput.addEventListener('input', updateResult);
    // 体重输入：记住到 localStorage（下次打开页面自动带出）
    bodyInput.addEventListener('input', () =>
    {
        localStorage.setItem('weight_converter_body', bodyInput.value);
        updateResult();
    });

    // 【教学：填入按钮 —— 把总重写进实际重量输入框】
    fillBtn.addEventListener('click', () =>
    {
        const mode = modeSelect.value;
        const total = convertWeight(
            mode,
            plateInput.value,
            barInput.value || defaultBar,
            bodyInput.value || defaultBody,
        );
        // 写进记录表单的重量输入框（id=weight-input）
        weightInput.value = roundToHalf(total);
    });

    // 初始计算一次（页面加载就有结果）
    updateResult();
}

// 【教学：DOMContentLoaded —— 等 HTML 全部加载完再绑定】
// 脚本放在 </body> 前时 DOM 已就绪，但规范写法仍监听此事件，
// 确保脚本放在 <head> 也能用。
document.addEventListener('DOMContentLoaded', initWeightConverter);

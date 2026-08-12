// ============================================================
// static/weight_converter.js —— 重量换算器（纯前端，M4 第 4 步）
// ============================================================
// 【教学说明】
// 训练中常要"片重 → 总重"换算。这是纯前端工具：
//   不碰后端、不查数据库，输入片重实时显示总重。
// 换算只是输入辅助，不是业务逻辑，所以不用 Rust 实现。
//
// 三种模式（与 exercises 表的 default_mode 一致）：
//   bar    杠铃：总重 = 杆重 + 2 × 片重（一侧一片）
//           杆重可选 0（倒蹲等无杆动作：片挂轴上，轴不称重）
//   support 自重：总重 = 体重 − 支撑量
//           （支撑器械标的是"帮你抵消多少体重"，如 90kg 体重
//             用 30kg 支撑做引体 → 实际负重 = 90 − 30 = 60kg）
//   std    标准：总重 = 片重（器械/哑铃/片直接读数）
//
// 【M4 修订：单位选择（kg/lb）在观测强度上】
// 计重方式精简为 bar/support/std 三种，原"标准lb"模式移除——
// lb 不再是模式，而是观测强度的【单位】：
//   观测强度旁的单位下拉选 kg 或 lb：
//     kg → 片重直接按 kg 参与公式
//     lb → 先 × 0.4536 归一化成 kg，再套模式公式
//   （1 磅 = 0.4536 kg；45 磅片 → 20.412kg）
// 计重方式里涉及的其他重量（杆重、体重）统一用 kg，不受单位影响。
// 单位选择不入库（观测强度本身也不入库），localStorage 记住偏好。
//
// 用法（HTML 里）：
//   <select id="mode-select">…</select>
//   <select id="unit-select">kg/lb</select>   ← 观测强度单位
//   <input id="plate-input" type="number">   ← bar/std 是片重；support 是支撑量
//   <input id="bar-input" type="number">     ← 仅 bar 模式显示
//   <input id="body-input" type="number">    ← 仅 support 模式显示（体重，localStorage 记住）
//   <span id="result"></span>
//   <input id="weight-input" type="number">  ← 实际强度（readonly，JS 实时自动写入）
//   页面把动作 bar_weight 渲染进 <body data-bar-weight="20">，
//   JS 从这里读初始杆重。
// ============================================================

// 【M5 修订：去掉"填入强度"按钮，改为自动更新】
// 之前是输入观测强度后点按钮才把总重写进 weight-input。
// 既然 JS 已实时算好，按钮纯属多余——直接随输入自动写入。
// 但注意保护：record_form 的 weight-input 初始可能有回显值
// （计划预设重量/上次记录重量），页面加载时 plate 为空，
// 此时不能覆盖回显值。所以 updateResult 只在 plate 有值时写入。

// 【教学：换算函数 —— 纯函数式（无副作用）】
// 输入 (mode, plate, bar, body, unit) → 输出总重。
// 纯函数 = 同样的输入永远同样的输出，不读外部状态。
// 这样方便单测、好推理。五个参数中：
//   mode   计重方式（bar/support/std）
//   plate  观测强度原始输入（单位是 unit，可能是 lb）
//   bar    杆重 kg（仅 bar 模式用，固定 kg）
//   body   体重 kg（仅 support 模式用，固定 kg）
//   unit   观测强度的单位（'kg' 或 'lb'）
// 内部先按 unit 把 plate 归一化成 kg，再套 mode 公式。
// 单位归一化是纯函数的第一步：lb → ×0.4536，kg → 原样。
function convertWeight(mode, plate, bar, body, unit)
{
    // 非数字输入 → 0（输入框清空时不算错）
    const raw = Number(plate) || 0;
    const barKg = Number(bar) || 0;
    const bodyKg = Number(body) || 0;
    // 【M4：观测强度按单位归一化成 kg】
    // 单位是 lb → ×0.4536；kg → 原样。杆重/体重不受影响（固定 kg）。
    const plateKg = unit === 'lb' ? raw * 0.4536 : raw;
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
            // 标准：观测强度就是总重（已归一化成 kg）
            return plateKg;
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

// 【M5 修订：逆换算 —— 实际强度 → 观测强度（纯函数）】
// 用户问题 2：record_form / plan_detail 的观测强度打开时为空。
// 数据库存的是实际强度（weight），但训练时最直观的参考是观测强度（片重）。
// 所以页面加载时，把 weight-input 的【回显值】逆换算成 plate 预填。
// 逆换算 = convertWeight 的反函数（解出 plateKg）：
//   bar:     总重 = 杆重 + 2×片重 → 片重 = (总重 - 杆重) / 2
//   support: 总重 = 体重 - 支撑量 → 支撑量 = 体重 - 总重
//   std:     总重 = 片重          → 片重 = 总重
// 单位 lb 时显示值 = kg ÷ 0.4536（与 convertWeight 的 lb→kg 归一化互逆）。
// 负数 clamp 到 0（杆重大于总重时会出现，如热身组只挂空杆）。
// 纯函数：不读 DOM、不写状态，方便与 convertWeight 成对测试。
function inverseConvert(mode, weight, bar, body, unit)
{
    const weightKg = Number(weight) || 0;
    const barKg = Number(bar) || 0;
    const bodyKg = Number(body) || 0;
    let plateKg = 0;
    switch (mode)
    {
        case 'bar':
            plateKg = (weightKg - barKg) / 2;
            break;
        case 'support':
            plateKg = bodyKg - weightKg;
            break;
        case 'std':
            plateKg = weightKg;
            break;
        default:
            plateKg = 0;
    }
    // 不可能负负重 → clamp 到 0
    plateKg = Math.max(0, plateKg);
    // lb 单位：显示值 = kg ÷ 0.4536（convertWeight 里 lb → ×0.4536，互逆）
    return unit === 'lb' ? plateKg / 0.4536 : plateKg;
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
    const unitSelect = document.getElementById('unit-select');
    const barRow = document.getElementById('bar-row');
    const barInput = document.getElementById('bar-input');
    const bodyRow = document.getElementById('body-row');
    const bodyInput = document.getElementById('body-input');
    const result = document.getElementById('result');
    const weightInput = document.getElementById('weight-input');

    // 从 <body data-bar-weight="20"> 读初始杆重（后端渲染进 HTML）
    const defaultBar = Number(document.body.dataset.barWeight) || 20;
    // 【M5 修订：体重优先取服务端全局体重（users.body_weight，首页维护）】
    // 用户问题 0：体重应在"一个地方维护成可编辑的通用变量"，record_form 自动获取。
    // 后端已把 users.body_weight 渲染进 body-input 的 value（见 record_form）。
    // 这里只在输入框为空时兜底用 localStorage（老用户习惯）→ 再兜底 70kg。
    const defaultBody = Number(bodyInput.value) ||
        Number(localStorage.getItem('weight_converter_body')) || 70;
    // 【M4：观测强度单位偏好从 localStorage 读】
    // 单位不入库（观测强度本身也不入库），只做前端换算辅助。
    // 记过一次后下次自动带出，不用每次重选（和体重同款机制）。
    const savedUnit = localStorage.getItem('weight_converter_unit');
    // 有历史偏好 → 覆盖 HTML 里的默认 selected（kg）
    if (savedUnit && (savedUnit === 'kg' || savedUnit === 'lb'))
    {
        unitSelect.value = savedUnit;
    }

    // 【教学：updateResult —— 读输入 → 换算 → 写显示】
    const updateResult = () =>
    {
        // 当前模式
        const mode = modeSelect.value;
        // bar 模式显示杆重行；support 模式显示体重行；其他模式都隐藏
        barRow.style.display = mode === 'bar' ? '' : 'none';
        bodyRow.style.display = mode === 'support' ? '' : 'none';
        // 换算（片重/支撑量 + 杆重 + 体重 + 单位）→ 总重
        const total = convertWeight(
            mode,
            plateInput.value,
            barInput.value || defaultBar,
            bodyInput.value || defaultBody,
            unitSelect.value || 'kg',
        );
        // 显示（0.5 的倍数）
        result.textContent = roundToHalf(total) + ' kg';
        // 【M5：自动写入实际强度（去掉"填入强度"按钮）】
        // 只在观测强度有输入时写——页面加载时 plate 为空，
        // 不能覆盖 weight-input 已有的回显值（计划预设/上次记录）。
        if (plateInput.value !== '')
        {
            weightInput.value = roundToHalf(total);
        }
    };

    // 【教学：事件监听 —— input 事件（每次输入都触发）】
    modeSelect.addEventListener('input', updateResult);
    plateInput.addEventListener('input', updateResult);
    barInput.addEventListener('input', updateResult);
    // 【M4：切换单位 → 立即重算 + 记住偏好】
    // 单位影响换算结果，切换时 updateResult 会重算。
    unitSelect.addEventListener('input', () =>
    {
        localStorage.setItem('weight_converter_unit', unitSelect.value);
        updateResult();
    });
    // 体重输入：记住到 localStorage（下次打开页面自动带出）
    // 【M5：服务端已有全局体重时，用户改了本地值 → 记 localStorage 覆盖】
    // 全局体重是"通用变量"（首页维护），这里保留 localStorage 同步是兼容
    // 老习惯——但下次打开 record_form 仍会被服务端全局体重覆盖。
    bodyInput.addEventListener('input', () =>
    {
        localStorage.setItem('weight_converter_body', bodyInput.value);
        updateResult();
    });

    // 【M5 修订：逆换算预填 —— 页面加载时只执行一次】
    // 用户问题 2：观测强度（plate）打开时为空，但 weight-input 有回显值
    // （当日值/计划值/上次值）。训练时最直观看片重，所以逆换算预填 plate。
    // ⚠️ 循环计算分析（用户问"会不会观测/实际强度循环计算"）：
    //   不会死循环。依赖是单向的：plate → weight（updateResult 只读 plate
    //   写 weight），weight 是 readonly 且没有任何监听器，不存在 weight→plate
    //   的反向路径。逆预填只在此处执行一次：写 plate 会触发 input 事件 →
    //   updateResult 重算 weight（结果 = 逆换算的逆 ≈ 原回显值，roundToHalf
    //   对齐）→ 写 weight 不再触发任何事件 → 链条终止。幂等。
    if (weightInput.value !== '' && plateInput.value === '')
    {
        plateInput.value = roundToHalf(inverseConvert(
            modeSelect.value,
            weightInput.value,
            barInput.value || defaultBar,
            bodyInput.value || defaultBody,
            unitSelect.value || 'kg',
        ));
    }

    // 初始计算一次（页面加载就有结果）
    updateResult();
}

// 【教学：DOMContentLoaded —— 等 HTML 全部加载完再绑定】
// 脚本放在 </body> 前时 DOM 已就绪，但规范写法仍监听此事件，
// 确保脚本放在 <head> 也能用。
document.addEventListener('DOMContentLoaded', initWeightConverter);

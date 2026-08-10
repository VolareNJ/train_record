#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
migrate_actions.py —— 从 migration_1.csv (GBK) 迁移动作库到生产数据库

需求:
  0. 同名动作（质量组/强度组）合并: 强度/容量按质量组，组数相加
  1. 二头/三头 → 手臂；腹1/腹2/腹3 → 核心
  2. 一甲/一乙 → 模板"一"；二甲/二乙 → 模板"二"；三甲/三乙 → 模板"三"
  3. 功能性动作部位 → 核心

用法:
  python3 migrate_actions.py --dry    # 预览（不写库）
  python3 migrate_actions.py          # 执行
"""
import csv
import re
import sqlite3
import sys

CSV_PATH = "/root/git-pkg/train_record/migration_1.csv"
DB_PATH = "/var/lib/train_record/train_record.db"
USER_ID = 1
PHASE_ID = 1

BAR_WEIGHTS = {"olympic": 20.0, "smith": 11.3, "short": 10.0}


def parse_intensity(raw):
    """解析强度列 → (default_mode, bar_weight)。未知格式返回 (None, None)。"""
    if not raw:
        return None, None
    s = raw.strip()
    m = re.match(r"body_weight\s*-\s*(\w+)\(([\d.]+)\)", s)
    if m:
        return "support", None
    m = re.match(r"bar\(([\d.]+)\s*,\s*(\w+)\)", s)
    if m:
        return "bar", BAR_WEIGHTS.get(m.group(2), 20.0)
    m = re.match(r"(lb2kg|std)\(([\d.]+)\)(?:\*(\d+))?", s)
    if m:
        return m.group(1), None
    return None, None


def to_int(s, default):
    try:
        return int(s)
    except (ValueError, TypeError):
        return default


def merge_items(items):
    """同名动作合并：强度/容量按质量组，组数相加。返回最终动作列表。"""
    groups = {}
    order = []
    for it in items:
        if it["name"] not in groups:
            groups[it["name"]] = []
            order.append(it["name"])
        groups[it["name"]].append(it)

    result = []
    for name in order:
        rows = groups[name]
        if len(rows) == 1:
            result.append(rows[0])
            continue
        # 多组：以质量组为基础（没有质量组则用第一行）
        quality = next((r for r in rows if r["group"] == "质量组"), None)
        base = dict(quality if quality else rows[0])
        # 组数相加（所有行的组数）
        total = sum(to_int(r["sets"], 0) for r in rows)
        if total > 0:
            base["sets"] = str(total)
        # 组别标记：合并后视为质量组
        base["group"] = "质量组(合并)"
        result.append(base)
    return result


def main():
    dry = "--dry" in sys.argv
    with open(CSV_PATH, encoding="gbk") as f:
        rows = list(csv.reader(f))

    templates = []  # [{tpl_name, items:[...]}]
    cur_tpl = None
    cur_part = None

    for row in rows:
        if not row or not row[0].strip():
            continue
        cell0 = row[0].strip()
        # 模板头：一甲/一乙/二甲...
        if len(row) >= 2 and not row[1].strip() and re.match(r"^[一二三][甲乙]$", cell0):
            cur_tpl = {"tpl_name": cell0[0], "items": []}
            templates.append(cur_tpl)
            cur_part = None
            continue
        # 部位头：腿,注释,组别,强度,...
        if len(row) >= 3 and row[1].strip() == "注释" and row[2].strip() == "组别":
            cur_part = cell0
            continue
        # 数据行
        if cur_tpl is None:
            continue
        name = cell0
        intensity = row[3].strip() if len(row) > 3 else ""
        sets = row[4].strip() if len(row) > 4 else ""
        reps = row[5].strip() if len(row) > 5 else ""
        # 跳过说明行（如"热身动作：..."，无强度无组数无容量）
        if not intensity and not sets and not reps:
            continue
        cur_tpl["items"].append({
            "name": name,
            "note": row[1].strip() if len(row) > 1 else "",
            "group": row[2].strip() if len(row) > 2 else "",
            "intensity": intensity,
            "sets": sets,
            "reps": reps,
            "key_points": row[10].strip() if len(row) > 10 else "",
            "body_part": cur_part,
        })

    PART_MAP = {
        "二头": "手臂",
        "三头": "手臂",
        "腹1": "核心",
        "腹2": "核心",
        "腹3": "核心",
        "功能性": "核心",  # 需求3：功能性 → 核心
    }

    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()
    existing = {r[0] for r in cur.execute(
        "SELECT name FROM exercises WHERE user_id=?", (USER_ID,)).fetchall()}

    # 幂等保护：模板已存在则跳过（防误跑重复插入）
    existing_tpls = {r[0] for r in cur.execute(
        "SELECT name FROM templates WHERE phase_id=?", (PHASE_ID,)).fetchall()}
    if existing_tpls:
        print("警告: phase 下已有模板", sorted(existing_tpls), "，为避免重复插入，中止。")
        print("如确需重迁，请先备份并清空 templates/template_items 再运行。")
        conn.close()
        return

    created_exercises = []
    summary = {}
    tpl_ids = {}
    for tpl in templates:
        items = merge_items(tpl["items"])
        tpl_name = tpl["tpl_name"]
        # 同名模板只创建一次（一甲/一乙 → "一"）
        if tpl_name not in tpl_ids:
            cur.execute(
                "INSERT INTO templates (phase_id, name, sort_order) VALUES (?,?,0)",
                (PHASE_ID, tpl_name))
            tpl_ids[tpl_name] = cur.lastrowid
            summary[tpl_name] = 0
        tpl_id = tpl_ids[tpl_name]
        sort = summary[tpl_name]
        for it in items:
            name = it["name"]
            if name not in existing:
                mode, bar_w = parse_intensity(it["intensity"])
                if mode is None:
                    mode = "std"
                sets = to_int(it["sets"], 3)
                reps = to_int(it["reps"], 8)
                kp = it["key_points"]
                if it["intensity"]:
                    kp = f"强度参考:{it['intensity']} | " + kp
                if it["note"]:
                    kp = f"备注:{it['note']} | " + kp
                body_part = PART_MAP.get(it["body_part"], it["body_part"])
                cur.execute(
                    "INSERT INTO exercises (user_id, name, body_part, default_mode, bar_weight,"
                    " default_sets, default_reps, key_points) VALUES (?,?,?,?,?,?,?,?)",
                    (USER_ID, name, body_part, mode, bar_w or 20.0, sets, reps, kp))
                ex_id = cur.lastrowid
                existing.add(name)
                created_exercises.append((name, body_part, mode, sets, reps, it["group"]))
            else:
                ex_id = cur.execute(
                    "SELECT id FROM exercises WHERE user_id=? AND name=?",
                    (USER_ID, name)).fetchone()[0]
            cur.execute(
                "INSERT INTO template_items (template_id, exercise_id, sort_order, plan_sets,"
                " plan_reps) VALUES (?,?,?,?,?)",
                (tpl_id, ex_id, sort, to_int(it["sets"], None), to_int(it["reps"], None)))
            sort += 1
        summary[tpl_name] = sort

    if not dry:
        conn.commit()
    conn.close()

    print(f"模板 {len(summary)} 个:")
    for n, c in summary.items():
        print(f"  {n}: {c} 个动作")
    print(f"新增动作 {len(created_exercises)} 个:")
    for name, part, mode, sets, reps, grp in created_exercises:
        print(f"  [{part}] {name}  mode={mode} {sets}组×{reps}次  ({grp})")
    if dry:
        print("\n[DRY-RUN] 未写库")


if __name__ == "__main__":
    main()

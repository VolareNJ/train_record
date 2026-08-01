print('wait......')

import pandas as pd
import datetime
from os import startfile, system
import matplotlib.pyplot as plt
import tkinter as tk
from pandastable import Table, TableModel

pd.set_option('display.max_columns', None)
pd.set_option('display.max_rows', None)
pd.set_option('display.expand_frame_repr', False)
pd.set_option('display.max_colwidth', None)  # 自动调整列宽以适应数据和行标题的长度

plt.rcParams['font.family'] = ['YouYuan'] # 设置中文字体
plt.rcParams['axes.unicode_minus'] = False # 解决负号显示问题
plt.style.use('dark_background')

'''
----------------------
useful calculation
----------------------
'''

method='phase1'
start_at = datetime.date(2023,1,1)
#method='convict_conditioning'

body_weight = 75  # 示例体重，可按需修改
olympic = 20
short = 9
smith = 11.3

print(f'Training System: {method}\nDuration: {datetime.date.today().__sub__(start_at).days}\nBody Weight: {body_weight}')

def bar(weight, bar_weight):
    return 2*weight+bar_weight

def support(support, body_weight=body_weight):
    return body_weight-abs(support)

def total(X):
    return {'bar':bar(X), 'support':support(X)}

def half_bar(weight, bar_weight):
    ori_total=bar(weight)
    tar_total=ori_total/2
    target_barbell=tar_total-bar_weight
    target_side=target_barbell/2
    return target_side

def half_support(support, body_weight=body_weight):
    ori_weight=support(support)
    tar_weight=ori_weight/2
    tar_support=body_weight-tar_weight
    return tar_support

def half(X):
    return {'bar':half_bar(X), 'support':half_support(X)}

def lb2kg(weight):
    return weight/2.2046

def std(weight):
    return weight

def calculate_RM(weight, reps, target_reps=1):
    # Epley formula to calculate 1RM
    one_RM = weight * (1 + reps / 30)
    if target_reps==1:
        return round(one_RM, 1)
    else:
        # Wathan formula to calculate MRM
        target_RM = one_RM / (1.67 - 0.067 * target_reps)
        return round(target_RM, 1)

'''
----------------------
core
----------------------
'''

# 创建一个函数，用于记录新的训练数据
def enter(exercise, weight, sets, reps, rest, feeling, strategy, date=None):
    # 构建新的训练数据
    if date == None:
        date = datetime.date.today()

    '''废弃代码
    if 'drop_set' in mode:
        weight = round(
            sum(v * w for v, w in zip(eval(weight), eval(reps))) / sum(eval(reps)),
            1)
        reps = round(sum(eval(reps))
                     /
                     sets)
    '''
    
    new_record = {'Date': date, 'Weight': weight, 'Sets': sets,
                  'Reps': reps, 'Rest': rest, 'Feeling': feeling, 'Strategy': strategy}

    # 检查是否存在对应的csv文件，若不存在则创建一个新的csv文件
    try:
        # records = pd.read_csv(f'all_data\\{method}\\{exercise}.csv').set_index('Date')
        records = call(exercise)
    except:
        records = pd.DataFrame(columns=new_record.keys()).set_index('Date')
    
    # 添加新的记录
    records = pd.concat([records, pd.DataFrame(new_record,index=['0']).set_index('Date')],
                        ignore_index=False)
    
    # 保存更新后的数据到本地csv文件
    records.to_csv(f'all_data\\{method}\\{exercise}.csv', index=True)
    print(exercise)
    print(call(exercise,aug=True,tail=3),'\n')


'''
----------------------
data manipulation
----------------------
'''

# 创建一个函数，用来批量导入位于table.xlsx的数据。其中，table.xlsx内的数据是被标准化的
def record(date=None):
    df=pd.read_excel('table.xlsx')
    for i in df.index:
        enter(exercise=df.loc[i,'区块'],
              weight=eval(df.loc[i,'强度']),
              sets=df.loc[i,'组数'],
              reps=df.loc[i,'容量'],
              rest=df.loc[i,'间隔'],
              feeling=df.loc[i,'感受'],
              strategy=df.loc[i,'后续'],
              date=date)

# 创建一个函数，用于删除某个日期某个动作的数据
def delete(exercise, date=datetime.date.today()):
    records = call(exercise)
    
    # 删除记录
    records.drop(str(date),inplace=True)
    
    # 保存更新后的数据到本地csv文件
    #这里index必须为True，否则日期数据会丢失
    records.to_csv(f'all_data\\{method}\\{exercise}.csv', index=True)
    print(exercise, date, 'deleted')

def reverse(date=datetime.date.today()):
    df=pd.read_excel('table.xlsx')
    exercises = df.iloc[:,0].unique() # 获取所有的动作
    for exercise in exercises: # 遍历所有的动作
        delete(exercise,date) # 对每个动作调用delete函数

'''
----------------------
tools
----------------------
'''

# 创建一个函数，用于分析某个动作的数据并绘制图表
def plot(exercise):
    # 读取本地csv文件中的数据
    records = call(exercise, True)
    
    # 按照日期升序排序
    records = records.sort_values(by='Date', ascending=True)

    # 绘制重量关于时间的plot
    plt.plot(records.index, records['Weight'], color='blue', label='Weight')

    # 绘制1RM关于时间的plot
    plt.plot(records.index, records['1RM'], color='red', label='1RM')
    
    plt.title(exercise)
    plt.xlabel('Date')
    plt.ylabel('kg')
    # 设置横坐标标签间隔为1个月
    plt.xticks(rotation=45, ha='right')
    plt.gca().xaxis.set_major_locator(plt.MaxNLocator(7))
    # 添加图例
    plt.legend()
    plt.show()

def call(exercise, aug=False, tail=None):
    if tail is None:
        df = pd.read_csv(f'all_data\\{method}\\{exercise}.csv',index_col='Date')
    else:
        df = pd.read_csv(f'all_data\\{method}\\{exercise}.csv',index_col='Date').tail(tail)
    if aug == True:
        df['1RM'] = calculate_RM(df['Weight'], df['Reps'], 1)
        df['2RM'] = calculate_RM(df['Weight'], df['Reps'], 2)
        df['3RM'] = calculate_RM(df['Weight'], df['Reps'], 3)
        df = df[['Weight','Sets','Reps','1RM','2RM','3RM','Rest','Feeling','Strategy']] 
    else:
        df = df[['Weight','Sets','Reps','Rest','Feeling','Strategy']] 
    return df

def edit(exercise):
    df=pd.read_csv(f'all_data\\{method}\\{exercise}.csv')
    
    # 创建一个tkinter窗口
    root = tk.Tk()
    root.title(f"{exercise}")

    # 创建一个PandasTable
    frame = tk.Frame(root)
    frame.pack(fill='both', expand=True)
    table = Table(frame, dataframe=df, showtoolbar=True, showstatusbar=True, font='YouYuan')
    table.show()

    # 启用编辑功能
    model = table.model
    model.df = df
    model.edited = True

    # 在关闭窗口时询问用户是否保存更改
    def on_close():
        if model.edited:
            result = tk.messagebox.askyesnocancel('保存更改', '是否保存编辑后的数据？')
            if result == True:
                df.update(model.df)
                df.to_csv(f'all_data\\{method}\\{exercise}.csv', index=False)
                root.destroy()
            elif result == False:
                root.destroy()

    # 监听窗口关闭事件
    root.protocol("WM_DELETE_WINDOW", on_close)
    
    # 运行tkinter窗口
    root.mainloop()

def open(exercise):
    startfile(f'all_data\\{method}\\{exercise}.csv')


print('ready')

#system("pause")

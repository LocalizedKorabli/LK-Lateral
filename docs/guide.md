## 项目特征

你需要实现一个Windows 64位桌面应用，名为“LK-Lateral”，旨在帮助Mir Korabley玩家安装Lesta Game Center（LGC）和Most Korabli的本地化。这两种应用的本地化都由文件覆盖安装，LGC的本地化一次性支持多种语言，Most本地化一次只能同时安装一种语言。

## 页面布局

多标签式界面：分为“主页”、“设置”、“关于”3个标签页。

以下为详情。

### 主页

左上角显示标题“应用”，右上角有两个按钮“刷新”和“自动扫描”。点击“刷新”时重新获取LGC和Most的状态，点击“自动扫描”则扫描设备上的LGC和Most实例，分类别保存所有扫描到的实例目录，并在每个分类（LGC、Most）扫描出多个实例时弹出窗口要求用户各选择一个。

下面则竖排显示“LGC”和“Most”两个磁贴，分别显示LGC、Most的信息和状态：应用路径、是否为最新版（最新、需要更新）、本地化安装状态（未安装、已安装：本地化语言、已修改：本地化语言）。两个磁贴下方分别有四个按钮：安装（本地化）、卸载（本地化）、启动和设置。点击设置时，弹出窗口，允许选择LGC或Most的路径，以及Most的目标本地化语言（先从远程json获取可用的本地化语言）。

### 设置

支持多语言：应用本身支持简中、繁中、英语、俄语、日语本地化，允许在“设置”标签中设定应用语言。

用户设置：在“设置”标签中还可以指定代理设置（使用系统代理、自定义代理——主机、端口、用户名、密码，后两者利用keyring存储），同时可以打开应用的数据目录。

### 关于

居中显示本应用的图标，在下方显示本应用的当前版本，并在更下方显示一个“更新”按钮。

## 实现细节

### 自动扫描

#### LGC

你需要通过注册表寻找Lesta Game Center的目录。

参考这段Python代码：
```
def _find_from_registry() -> Optional[Path]:
    log("Scanning LGC registries...")
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r'Software\Classes\lgc\DefaultIcon') as key:
            lgc_dir_str, _ = winreg.QueryValueEx(key, '')

        if ',' in lgc_dir_str:
            lgc_dir_str = lgc_dir_str.split(',')[0]

        lgc_dir = Path(lgc_dir_str)
        if is_valid_lgc_instance(lgc_dir):
            return lgc_dir

    except FileNotFoundError:
        log("LGC registry key not found. Skipping registry scan.")
    except Exception as e:
        log(f"Error scanning registry: {e}")
    default_dir = Path(r"C:\ProgramData\Lesta\GameCenter")
    return default if is_valid_lgc_instance(default_dir) else None

def is_valid_lgc_instance(path: Path) -> bool:
    try:
        if not path.is_dir():
            return False
        if not (path / 'lgc.exe').is_file():
            return False
    except Exception:
        return False
    return True
```

#### Most

你需要通过注册表寻找Most Korabli的目录。

参考这段Python代码：
```
def _find_from_registry() -> Optional[Path]:
    log("Scanning LGC registries...")
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\Lesta\Most Korabli\Data') as key:
            most_dir_str, _ = winreg.QueryValueEx(key, 'InstallPath')

        most_dir = Path(most_dir_str)
        if is_valid_most_instance(most_dir):
            return most_dir

    except FileNotFoundError:
        log("Most registry key not found. Skipping registry scan.")
    except Exception as e:
        log(f"Error scanning registry: {e}")
    default_dir = Path(r"C:\Program Files\Lesta\Most Korabli")
    return default if is_valid_most_instance(default_dir) else None

def is_valid_most_instance(path: Path) -> bool:
    try:
        if not path.is_dir():
            return False
        if not (path / 'Korabli.Most.exe').is_file():
            return False
    except Exception:
        return False
    return True
```

### 获取应用本体版本

#### LGC

LGC目录下lgc.exe的产品版本。

#### Most

Most目录下Korabli.Most.exe的产品版本。

### 获取本地化版本及下载本地化

#### LGC

访问https://localizedkorabli.org/metadata/lgc/l10n.json。
格式形如：
```
{
    "path": "https://dl.localizedkorabli.org/lateral/lgc/lgc_l10n.7z",
    "version": "26.1.0",
    "supported_lgc_version": "26.00.02.1046"
}
```
读取应用本体版本和上述远程supported_lgc_version：
- 前者小于后者时，提醒用户先更新LGC本体
- 前者等于后者时，从path下载安装包7z文件
- 前者大于后者时，告知用户目前本地化还未就绪，耐心等待发布

#### Most

访问https://localizedkorabli.org/metadata/most/l10n.json。
格式形如：
```
[
    {
        "id": "zh_CN",
        "name": "简体中文",
        "l10n_app": {
            "path": "https://dl.localizedkorabli.org/lateral/most/zh_CN/most_l10n_app.7z",
            "version": "26.1.0",
            "supported_most_version": "3.1.5.3"
        },
        "l10n_mods": {
            "path": "https://dl.localizedkorabli.org/lateral/most/zh_CN/most_l10n_mods.7z",
            "version": "26.1.0"
        }
    }
]
```
先获取上述列表中的所有项，并为用户显示其name，用户选择某项后，获取该项的l10n_app和l10n_mods项。
对于l10n_app，读取应用本体版本和上述远程supported_most_version：
- 前者小于后者时，提醒用户先更新Most本体
- 前者等于后者时，从path下载安装包7z文件
- 前者大于后者时，告知用户目前本地化还未就绪，耐心等待发布，且不下载后续的l10n_mods
对于l10n_mods，从path下载安装包7z文件。

### 安装本地化

对于LGC、Most（app和mods），本地化的安装方式都是将7z文件解压到LGC/Most的安装目录。

我们在LGC/Most安装目录下创建lateral文件夹，在其中的inst_info.json中记录本地化安装状态——包括本地化版本（Most同时记录l10n_app和l10n_mods版本）、适用的LGC/Most版本、所有在解压时被复制进来的文件的路径和SHA256值。

不过由于提供卸载功能，我们也需要在"lateral/backups/日期/files/"备份所有被替换文件并在"lateral/backups/日期/backup_info.json"记录其SHA256值。

另外在inst_info.json中记录本次安装时备份的目录"lateral/backups/日期/"，以供恢复备份。

### 缓存与版本检查

当远程本地化版本和本地版本无差别，且本地已安装文件能通过inst_info.json的SHA256检查时，不进行下载。
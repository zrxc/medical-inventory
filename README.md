# 医疗产品进销存记录工具

这是一个把线下 Excel 进销存记录搬到本地工具里的进销存小系统。当前包含 Rust/egui 桌面版，以及基于 Vue3 + Vite + Element Plus 的前端版；口径偏“记录系统”，重点是把产品、订单、退货和金额明细录入、查询、导出。

## 主要功能

- 产品基础信息维护：产品名称、规格/型号、品牌、进货金额、出库金额、图片路径、描述；Vue 前端支持从 Excel 批量导入。
- 订单记录维护：产品、货号、订购日期、出库日期、订货单位、订货人、数量、销售金额、成本、毛利、备注；Vue 前端支持从 Excel 批量导入。
- 退货记录维护：关联订单后自动带出产品和客户信息，并回写订单已退数量；Vue 前端支持从 Excel 批量导入。
- 看板统计：产品数、订单数、退货记录、今日出库、今日退货、累计金额和日期趋势。
- 本地数据保存：数据写入本机用户数据目录，并自动创建备份。
- 导入导出：Vue 前端支持导出 Excel 导入模板、Excel 导入产品/订单/退货，并保留 CSV 导出用于核对。
- 出库单导出：订单列表可生成文本版出库单。

## 运行

Vue3 前端：

```powershell
cd frontend
npm install
npm run dev
```

前端生产构建：

```powershell
cd frontend
npm run build
```

前端 Windows exe 打包：

```powershell
cd frontend
npm run build:exe
```

产物默认输出到：

```text
frontend\release
```

Rust/egui 桌面版：

```powershell
cargo run
```

## 桌面端打包

Windows 便携版打包：

```powershell
.\scripts\package-windows.ps1
```

脚本会先执行 `cargo build --release`，然后生成：

```text
dist\medical-inventory-windows-x64.zip
```

解压后可直接运行 `Start.bat` 或 `MedicalInventory.exe`。发布版不会额外弹出控制台窗口，数据仍保存在系统本地数据目录。

常用检查命令：

```powershell
cargo fmt
cargo test
cargo clippy -- -D warnings
cargo check
```

## 数据位置

应用启动后会在界面底部显示数据文件、备份目录和导出目录。默认位置来自系统本地数据目录，例如 Windows 上通常类似：

```text
%LOCALAPPDATA%\medical_inventory\inventory.json
%LOCALAPPDATA%\medical_inventory\backups
%LOCALAPPDATA%\medical_inventory\exports
%LOCALAPPDATA%\medical_inventory\outbound_notes
```

## 使用建议

- 先在“产品管理”录入常用产品，再在“下单管理”选择已登记产品下单。
- Vue 前端里的“导入Excel”可读取按模板整理好的产品、订单、退货工作表；相同产品名称、品牌、规格/型号的产品记录会自动跳过。
- Vue 前端导入订单时，如果能按名称和品牌匹配到已有产品，会自动回填关联产品。
- Vue 前端导入退货时会按“关联订单ID”匹配订单，导入成功后会同步回写原订单已退数量。
- Excel 导入时金额类空单元格会按 `0` 处理，订单和退货数量必须大于 `0`。
- Vue 前端的产品、订单、退货新增/编辑统一使用弹窗表单，不再和列表并排常驻。
- 退货登记只会列出当前还有可退数量的订单，已全部退完的订单不会再出现在退货下拉框中。
- 订单和退货表单里的“重算金额”会按数量、销售单价、成本单价和返现重新计算销售总价、成本总价和毛利。
- 搜索支持产品描述、订单备注、退货备注等自由文本，适合承接原 Excel 里的补充说明。
- 日期支持 `2026-05-30`、`2026/5/30`、`2026.5.30` 等常见 Excel 粘贴格式，保存时会统一成 `YYYY-MM-DD`。
- 金额支持 `¥1,234.50`、`￥1，234.50` 这类粘贴格式；折扣支持 `80%` 这类百分号格式。
- CSV 导出文件会保存在 `exports` 目录，可分别导出产品、订单、退货，也可一键导出全部，可直接用 Excel 打开核对。
- Vue 前端“Excel模板”会生成包含产品、订单、退货三个工作表的 `.xlsx` 文件，适合先按系统字段整理原 Excel 数据。
- 手动备份可随时在底部状态栏创建；每次保存前也会自动备份旧数据。

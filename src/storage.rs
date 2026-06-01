use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use chrono::Local;
use uuid::Uuid;

use crate::models::{
    AppData, Order, Product, ReturnRecord, format_money, normalize_date, now_string, parse_f64,
};

const PRODUCT_CSV_HEADERS: &[&str] = &[
    "产品名称",
    "进货金额",
    "出库金额",
    "规格/型号",
    "品牌",
    "图片路径",
    "描述",
    "创建时间",
];

const ORDER_CSV_HEADERS: &[&str] = &[
    "产品名称",
    "货号",
    "订购时间",
    "出库日期",
    "订货单位",
    "订货人",
    "品牌",
    "单位",
    "目录价",
    "数量",
    "销售总价",
    "返现",
    "成本折扣",
    "成本单价",
    "成本总价",
    "售价折扣",
    "销售单价",
    "毛利",
    "备注",
    "已退数量",
    "创建时间",
];

const RETURN_CSV_HEADERS: &[&str] = &[
    "产品名称",
    "货号",
    "退货日期",
    "订货单位",
    "订货人",
    "品牌",
    "单位",
    "目录价",
    "数量",
    "销售总价",
    "返现",
    "成本折扣",
    "成本单价",
    "成本总价",
    "售价折扣",
    "销售单价",
    "毛利",
    "备注",
    "关联订单ID",
    "创建时间",
];

pub struct LoadOutcome {
    pub data: AppData,
    pub notice: String,
}

pub struct Storage {
    data_dir: PathBuf,
    data_file: PathBuf,
    backup_dir: PathBuf,
    max_backups: usize,
}

impl Storage {
    pub fn new() -> Self {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        Self::from_base_dir(base_dir)
    }

    fn from_base_dir(base_dir: PathBuf) -> Self {
        let data_dir = base_dir.join("medical_inventory");
        let data_file = data_dir.join("inventory.json");
        let backup_dir = data_dir.join("backups");

        Self {
            data_dir,
            data_file,
            backup_dir,
            max_backups: 60,
        }
    }

    pub fn data_file(&self) -> &Path {
        &self.data_file
    }

    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    pub fn export_dir(&self) -> PathBuf {
        self.data_dir.join("exports")
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
        fs::create_dir_all(&self.backup_dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
        Ok(())
    }

    pub fn load(&self) -> LoadOutcome {
        if let Err(err) = self.ensure_dirs() {
            return LoadOutcome {
                data: AppData::default(),
                notice: err,
            };
        }

        if self.data_file.exists() {
            match fs::read_to_string(&self.data_file) {
                Ok(content) => match serde_json::from_str::<AppData>(&content) {
                    Ok(data) => {
                        return LoadOutcome {
                            data,
                            notice: format!("已加载本地数据: {}", self.data_file.display()),
                        };
                    }
                    Err(err) => {
                        if let Some(data) = self.load_from_backup() {
                            return self.recovered_outcome(
                                "主数据文件损坏",
                                &err.to_string(),
                                data,
                            );
                        }
                    }
                },
                Err(err) => {
                    if let Some(data) = self.load_from_backup() {
                        return self.recovered_outcome(
                            "主数据文件读取失败",
                            &err.to_string(),
                            data,
                        );
                    }
                }
            }
        }

        LoadOutcome {
            data: AppData::default(),
            notice: format!(
                "首次运行。数据将保存到 {}，自动备份保存到 {}",
                self.data_file.display(),
                self.backup_dir.display()
            ),
        }
    }

    pub fn save(&self, data: &AppData) -> Result<(), String> {
        self.ensure_dirs()?;

        let serialized =
            serde_json::to_vec_pretty(data).map_err(|e| format!("序列化数据失败: {e}"))?;

        if self.data_file.exists() {
            let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
            let backup_file = self.backup_dir.join(format!("inventory-{stamp}.json"));
            fs::copy(&self.data_file, backup_file).map_err(|e| format!("创建自动备份失败: {e}"))?;
        }

        self.write_data_file(&serialized)?;
        self.prune_backups()?;
        Ok(())
    }

    pub fn create_manual_backup(&self, data: &AppData) -> Result<PathBuf, String> {
        self.ensure_dirs()?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
        let backup_file = self.backup_dir.join(format!("manual-{stamp}.json"));
        let serialized =
            serde_json::to_vec_pretty(data).map_err(|e| format!("序列化备份数据失败: {e}"))?;
        fs::write(&backup_file, serialized).map_err(|e| format!("写入手动备份失败: {e}"))?;
        Ok(backup_file)
    }

    pub fn create_outbound_note(&self, order: &Order) -> Result<PathBuf, String> {
        self.ensure_dirs()?;
        let note_dir = self.data_dir.join("outbound_notes");
        fs::create_dir_all(&note_dir).map_err(|e| format!("创建出库单目录失败: {e}"))?;

        let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
        let order_id = order.id.to_string();
        let short_id = order_id.get(..8).unwrap_or(&order_id);
        let note_file = note_dir.join(format!("outbound-{stamp}-{short_id}.txt"));

        fs::write(&note_file, self.render_outbound_note(order))
            .map_err(|e| format!("写入出库单失败: {e}"))?;
        Ok(note_file)
    }

    pub fn export_products_csv(&self, products: &[Product]) -> Result<PathBuf, String> {
        let rows = products
            .iter()
            .map(|product| {
                csv_row(&[
                    &product.name,
                    &format_money(product.purchase_price),
                    &format_money(product.outbound_price),
                    &product.specification,
                    &product.brand,
                    &product.image_path,
                    &product.description,
                    &product.created_at,
                ])
            })
            .collect::<Vec<_>>();

        self.write_csv_export("products", &csv_with_header(PRODUCT_CSV_HEADERS, &rows))
    }

    pub fn export_orders_csv(&self, orders: &[Order]) -> Result<PathBuf, String> {
        let rows = orders
            .iter()
            .map(|order| {
                csv_row(&[
                    &order.product_name,
                    &order.item_no,
                    &order.order_time,
                    &order.delivery_date,
                    &order.customer_unit,
                    &order.customer_name,
                    &order.brand,
                    &order.unit,
                    &format_money(order.catalog_price),
                    &format_money(order.quantity),
                    &format_money(order.invoice_total),
                    &format_money(order.cashback),
                    &format_money(order.cost_discount),
                    &format_money(order.cost_unit_price),
                    &format_money(order.cost_total),
                    &format_money(order.sale_discount),
                    &format_money(order.invoice_unit_price),
                    &format_money(order.gross_profit),
                    &order.remark,
                    &format_money(order.returned_quantity),
                    &order.created_at,
                ])
            })
            .collect::<Vec<_>>();

        self.write_csv_export("orders", &csv_with_header(ORDER_CSV_HEADERS, &rows))
    }

    pub fn export_returns_csv(&self, returns: &[ReturnRecord]) -> Result<PathBuf, String> {
        let rows = returns
            .iter()
            .map(|record| {
                csv_row(&[
                    &record.product_name,
                    &record.item_no,
                    &record.return_time,
                    &record.customer_unit,
                    &record.customer_name,
                    &record.brand,
                    &record.unit,
                    &format_money(record.catalog_price),
                    &format_money(record.quantity),
                    &format_money(record.invoice_total),
                    &format_money(record.cashback),
                    &format_money(record.cost_discount),
                    &format_money(record.cost_unit_price),
                    &format_money(record.cost_total),
                    &format_money(record.sale_discount),
                    &format_money(record.invoice_unit_price),
                    &format_money(record.gross_profit),
                    &record.remark,
                    &record.source_order_id.to_string(),
                    &record.created_at,
                ])
            })
            .collect::<Vec<_>>();

        self.write_csv_export("returns", &csv_with_header(RETURN_CSV_HEADERS, &rows))
    }

    pub fn export_all_csv(&self, data: &AppData) -> Result<Vec<PathBuf>, String> {
        Ok(vec![
            self.export_products_csv(&data.products)?,
            self.export_orders_csv(&data.orders)?,
            self.export_returns_csv(&data.returns)?,
        ])
    }

    pub fn export_csv_templates(&self) -> Result<Vec<PathBuf>, String> {
        Ok(vec![
            self.write_csv_export(
                "template-products",
                &csv_with_header(PRODUCT_CSV_HEADERS, &[]),
            )?,
            self.write_csv_export("template-orders", &csv_with_header(ORDER_CSV_HEADERS, &[]))?,
            self.write_csv_export(
                "template-returns",
                &csv_with_header(RETURN_CSV_HEADERS, &[]),
            )?,
        ])
    }

    pub fn read_products_csv(&self, path: &Path) -> Result<Vec<Product>, String> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(path)
            .map_err(|e| format!("读取产品CSV失败: {e}"))?;
        let headers = reader
            .headers()
            .map_err(|e| format!("读取产品CSV表头失败: {e}"))?
            .clone();
        let mut products = Vec::new();

        for (index, record) in reader.records().enumerate() {
            let record = record.map_err(|e| format!("读取产品CSV第 {} 行失败: {e}", index + 2))?;
            let name = csv_field(&headers, &record, "产品名称").trim();
            if name.is_empty() {
                return Err(format!("产品CSV第 {} 行缺少产品名称", index + 2));
            }

            let purchase_price =
                parse_csv_non_negative_number(&headers, &record, "进货金额", index + 2, "产品")?;
            let outbound_price =
                parse_csv_non_negative_number(&headers, &record, "出库金额", index + 2, "产品")?;
            let created_at = csv_field(&headers, &record, "创建时间").trim();

            products.push(Product {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                purchase_price,
                outbound_price,
                specification: csv_field(&headers, &record, "规格/型号").trim().to_string(),
                brand: csv_field(&headers, &record, "品牌").trim().to_string(),
                image_path: csv_field(&headers, &record, "图片路径").trim().to_string(),
                description: csv_field(&headers, &record, "描述").trim().to_string(),
                created_at: if created_at.is_empty() {
                    now_string()
                } else {
                    created_at.to_string()
                },
            });
        }

        Ok(products)
    }

    pub fn read_orders_csv(&self, path: &Path, products: &[Product]) -> Result<Vec<Order>, String> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(path)
            .map_err(|e| format!("读取订单CSV失败: {e}"))?;
        let headers = reader
            .headers()
            .map_err(|e| format!("读取订单CSV表头失败: {e}"))?
            .clone();
        let mut orders = Vec::new();

        for (index, record) in reader.records().enumerate() {
            let record = record.map_err(|e| format!("读取订单CSV第 {} 行失败: {e}", index + 2))?;
            let product_name = csv_field(&headers, &record, "产品名称").trim();
            if product_name.is_empty() {
                return Err(format!("订单CSV第 {} 行缺少产品名称", index + 2));
            }

            let order_time = normalize_date(csv_field(&headers, &record, "订购时间"))
                .unwrap_or_else(|| csv_field(&headers, &record, "订购时间").trim().to_string());
            let delivery_date = normalize_date(csv_field(&headers, &record, "出库日期"))
                .unwrap_or_else(|| csv_field(&headers, &record, "出库日期").trim().to_string());
            let quantity = parse_csv_positive_number(&headers, &record, "数量", index + 2, "订单")?;

            orders.push(Order {
                id: Uuid::new_v4(),
                product_id: resolve_product_id_for_import(
                    products,
                    product_name,
                    csv_field(&headers, &record, "品牌").trim(),
                ),
                product_name: product_name.to_string(),
                item_no: csv_field(&headers, &record, "货号").trim().to_string(),
                order_time,
                delivery_date,
                customer_unit: csv_field(&headers, &record, "订货单位").trim().to_string(),
                customer_name: csv_field(&headers, &record, "订货人").trim().to_string(),
                brand: csv_field(&headers, &record, "品牌").trim().to_string(),
                unit: csv_field(&headers, &record, "单位").trim().to_string(),
                catalog_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "目录价",
                    index + 2,
                    "订单",
                )?,
                quantity,
                invoice_total: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "销售总价",
                    index + 2,
                    "订单",
                )?,
                invoice_status: String::new(),
                is_shipped: true,
                cashback: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "返现",
                    index + 2,
                    "订单",
                )?,
                cost_discount: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本折扣",
                    index + 2,
                    "订单",
                )?,
                cost_unit_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本单价",
                    index + 2,
                    "订单",
                )?,
                cost_total: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本总价",
                    index + 2,
                    "订单",
                )?,
                sale_discount: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "售价折扣",
                    index + 2,
                    "订单",
                )?,
                invoice_unit_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "销售单价",
                    index + 2,
                    "订单",
                )?,
                gross_profit: parse_csv_number(&headers, &record, "毛利", index + 2, "订单")?,
                remark: csv_field(&headers, &record, "备注").trim().to_string(),
                invoice_no: String::new(),
                is_paid: true,
                paid_time: String::new(),
                returned_quantity: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "已退数量",
                    index + 2,
                    "订单",
                )?,
                created_at: {
                    let created_at = csv_field(&headers, &record, "创建时间").trim();
                    if created_at.is_empty() {
                        now_string()
                    } else {
                        created_at.to_string()
                    }
                },
            });
        }

        Ok(orders)
    }

    pub fn read_returns_csv(
        &self,
        path: &Path,
        orders: &[Order],
        products: &[Product],
    ) -> Result<Vec<ReturnRecord>, String> {
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(path)
            .map_err(|e| format!("读取退货CSV失败: {e}"))?;
        let headers = reader
            .headers()
            .map_err(|e| format!("读取退货CSV表头失败: {e}"))?
            .clone();
        let mut returns = Vec::new();

        for (index, record) in reader.records().enumerate() {
            let record = record.map_err(|e| format!("读取退货CSV第 {} 行失败: {e}", index + 2))?;
            let product_name = csv_field(&headers, &record, "产品名称").trim();
            if product_name.is_empty() {
                return Err(format!("退货CSV第 {} 行缺少产品名称", index + 2));
            }
            let source_order_id_text = csv_field(&headers, &record, "关联订单ID").trim();
            let source_order_id = if source_order_id_text.is_empty() {
                resolve_order_id_for_import(
                    orders,
                    product_name,
                    csv_field(&headers, &record, "货号").trim(),
                    csv_field(&headers, &record, "订货单位").trim(),
                    csv_field(&headers, &record, "订货人").trim(),
                    csv_field(&headers, &record, "品牌").trim(),
                    csv_field(&headers, &record, "单位").trim(),
                )
                .ok_or_else(|| format!("退货CSV第 {} 行无法匹配关联订单", index + 2))?
            } else {
                let source_order_id = Uuid::parse_str(source_order_id_text)
                    .map_err(|err| format!("退货CSV第 {} 行关联订单ID无效: {err}", index + 2))?;
                if !orders.iter().any(|order| order.id == source_order_id) {
                    return Err(format!("退货CSV第 {} 行关联订单不存在", index + 2));
                }
                source_order_id
            };
            let return_time = normalize_date(csv_field(&headers, &record, "退货日期"))
                .unwrap_or_else(|| csv_field(&headers, &record, "退货日期").trim().to_string());
            let quantity = parse_csv_positive_number(&headers, &record, "数量", index + 2, "退货")?;

            returns.push(ReturnRecord {
                id: Uuid::new_v4(),
                source_order_id,
                product_id: resolve_product_id_for_import(
                    products,
                    product_name,
                    csv_field(&headers, &record, "品牌").trim(),
                ),
                product_name: product_name.to_string(),
                item_no: csv_field(&headers, &record, "货号").trim().to_string(),
                return_time,
                customer_unit: csv_field(&headers, &record, "订货单位").trim().to_string(),
                customer_name: csv_field(&headers, &record, "订货人").trim().to_string(),
                brand: csv_field(&headers, &record, "品牌").trim().to_string(),
                unit: csv_field(&headers, &record, "单位").trim().to_string(),
                catalog_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "目录价",
                    index + 2,
                    "退货",
                )?,
                quantity,
                invoice_total: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "销售总价",
                    index + 2,
                    "退货",
                )?,
                invoice_status: String::new(),
                is_shipped: true,
                cashback: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "返现",
                    index + 2,
                    "退货",
                )?,
                cost_discount: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本折扣",
                    index + 2,
                    "退货",
                )?,
                cost_unit_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本单价",
                    index + 2,
                    "退货",
                )?,
                cost_total: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "成本总价",
                    index + 2,
                    "退货",
                )?,
                sale_discount: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "售价折扣",
                    index + 2,
                    "退货",
                )?,
                invoice_unit_price: parse_csv_non_negative_number(
                    &headers,
                    &record,
                    "销售单价",
                    index + 2,
                    "退货",
                )?,
                gross_profit: parse_csv_number(&headers, &record, "毛利", index + 2, "退货")?,
                remark: csv_field(&headers, &record, "备注").trim().to_string(),
                invoice_no: String::new(),
                is_paid: true,
                paid_time: String::new(),
                created_at: {
                    let created_at = csv_field(&headers, &record, "创建时间").trim();
                    if created_at.is_empty() {
                        now_string()
                    } else {
                        created_at.to_string()
                    }
                },
            });
        }

        Ok(returns)
    }

    fn recovered_outcome(&self, prefix: &str, error: &str, data: AppData) -> LoadOutcome {
        let notice = match serde_json::to_vec_pretty(&data)
            .map_err(|e| format!("序列化恢复数据失败: {e}"))
            .and_then(|serialized| self.write_data_file(&serialized))
        {
            Ok(()) => format!(
                "{prefix}，已从最近可用备份恢复并写回主数据文件。错误: {error}；备份目录: {}",
                self.backup_dir.display()
            ),
            Err(restore_err) => format!(
                "{prefix}，已从最近可用备份恢复到内存，但写回主数据文件失败: {restore_err}。原始错误: {error}；备份目录: {}",
                self.backup_dir.display()
            ),
        };

        LoadOutcome { data, notice }
    }

    fn write_data_file(&self, serialized: &[u8]) -> Result<(), String> {
        let temp_file = self.data_dir.join("inventory.json.tmp");
        let mut file =
            fs::File::create(&temp_file).map_err(|e| format!("创建临时文件失败: {e}"))?;
        file.write_all(serialized)
            .map_err(|e| format!("写入临时文件失败: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("同步临时文件失败: {e}"))?;
        drop(file);

        self.replace_data_file(&temp_file)
    }

    fn replace_data_file(&self, temp_file: &Path) -> Result<(), String> {
        let old_file = self.data_dir.join("inventory.json.old");
        if old_file.exists() {
            fs::remove_file(&old_file).map_err(|e| format!("清理旧临时数据文件失败: {e}"))?;
        }

        let had_existing_file = self.data_file.exists();
        if had_existing_file {
            fs::rename(&self.data_file, &old_file)
                .map_err(|e| format!("暂存旧数据文件失败: {e}"))?;
        }

        if let Err(err) = fs::rename(temp_file, &self.data_file) {
            if had_existing_file && old_file.exists() {
                let _ = fs::rename(&old_file, &self.data_file);
            }
            return Err(format!("保存数据失败: {err}"));
        }

        if old_file.exists() {
            fs::remove_file(&old_file).map_err(|e| format!("清理旧数据文件失败: {e}"))?;
        }

        Ok(())
    }

    fn render_outbound_note(&self, order: &Order) -> String {
        format!(
            "\
医疗产品出库单

生成时间: {}
订单ID: {}
产品名称: {}
货号: {}
品牌: {}
订货单位: {}
订货人: {}
订购时间: {}
出库日期: {}
数量: {} {}
当前出库数量: {} {}
已退数量: {} {}
销售总价: ¥{}
当前出库金额: ¥{}
备注: {}
",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            order.id,
            order.product_name,
            blank_or_dash(&order.item_no),
            blank_or_dash(&order.brand),
            blank_or_dash(&order.customer_unit),
            blank_or_dash(&order.customer_name),
            blank_or_dash(&order.order_time),
            blank_or_dash(&order.delivery_date),
            format_money(order.quantity),
            order.unit,
            format_money(order.current_outbound_quantity()),
            order.unit,
            format_money(order.returned_quantity),
            order.unit,
            format_money(order.invoice_total),
            format_money(order.current_outbound_amount()),
            blank_or_dash(&order.remark)
        )
    }

    fn write_csv_export(&self, name: &str, content: &str) -> Result<PathBuf, String> {
        self.ensure_dirs()?;
        let export_dir = self.export_dir();
        fs::create_dir_all(&export_dir).map_err(|e| format!("创建导出目录失败: {e}"))?;

        let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
        let export_file = export_dir.join(format!("{name}-{stamp}.csv"));
        fs::write(&export_file, content).map_err(|e| format!("写入导出文件失败: {e}"))?;
        Ok(export_file)
    }

    fn load_from_backup(&self) -> Option<AppData> {
        let mut entries = fs::read_dir(&self.backup_dir)
            .ok()?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();

        entries.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        entries.reverse();

        for entry in entries {
            let Ok(content) = fs::read_to_string(entry.path()) else {
                continue;
            };
            if let Ok(data) = serde_json::from_str::<AppData>(&content) {
                return Some(data);
            }
        }

        None
    }

    fn prune_backups(&self) -> Result<(), String> {
        let mut entries = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("读取备份目录失败: {e}"))?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("inventory-")
            })
            .collect::<Vec<_>>();

        if entries.len() <= self.max_backups {
            return Ok(());
        }

        entries.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        let remove_count = entries.len().saturating_sub(self.max_backups);
        for entry in entries.into_iter().take(remove_count) {
            fs::remove_file(entry.path()).map_err(|e| format!("清理旧备份失败: {e}"))?;
        }

        Ok(())
    }
}

fn blank_or_dash(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() { "-" } else { value }
}

fn csv_with_header(header: &[&str], rows: &[String]) -> String {
    let mut content = String::from("\u{feff}");
    content.push_str(&csv_row(header));
    for row in rows {
        content.push_str(row);
    }
    content
}

fn csv_field<'a>(
    headers: &csv::StringRecord,
    record: &'a csv::StringRecord,
    name: &str,
) -> &'a str {
    headers
        .iter()
        .position(|header| header.trim_start_matches('\u{feff}') == name)
        .and_then(|index| record.get(index))
        .unwrap_or("")
}

fn parse_csv_number(
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    label: &str,
    row: usize,
    csv_name: &str,
) -> Result<f64, String> {
    let value = csv_field(headers, record, label).trim();
    if value.is_empty() {
        return Ok(0.0);
    }
    parse_f64(value).map_err(|err| format!("{csv_name}CSV第 {row} 行{label}无效: {err}"))
}

fn parse_csv_non_negative_number(
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    label: &str,
    row: usize,
    csv_name: &str,
) -> Result<f64, String> {
    let value = parse_csv_number(headers, record, label, row, csv_name)?;
    if value < 0.0 {
        Err(format!("{csv_name}CSV第 {row} 行{label}不能小于 0"))
    } else {
        Ok(value)
    }
}

fn parse_csv_positive_number(
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    label: &str,
    row: usize,
    csv_name: &str,
) -> Result<f64, String> {
    let value = parse_csv_number(headers, record, label, row, csv_name)?;
    if value <= 0.0 {
        Err(format!("{csv_name}CSV第 {row} 行{label}必须大于 0"))
    } else {
        Ok(value)
    }
}

fn resolve_product_id_for_import(products: &[Product], name: &str, brand: &str) -> Option<Uuid> {
    let mut matches = products.iter().filter(|product| {
        product.name.trim() == name.trim() && product.brand.trim() == brand.trim()
    });
    let first = matches.next()?;
    if matches.next().is_none() {
        Some(first.id)
    } else {
        None
    }
}

fn resolve_order_id_for_import(
    orders: &[Order],
    product_name: &str,
    item_no: &str,
    customer_unit: &str,
    customer_name: &str,
    brand: &str,
    unit: &str,
) -> Option<Uuid> {
    let mut matches = orders.iter().filter(|order| {
        order.product_name.trim() == product_name.trim()
            && order.item_no.trim() == item_no.trim()
            && order.customer_unit.trim() == customer_unit.trim()
            && order.customer_name.trim() == customer_name.trim()
            && order.brand.trim() == brand.trim()
            && order.unit.trim() == unit.trim()
    });
    let first = matches.next()?;
    if matches.next().is_none() {
        Some(first.id)
    } else {
        None
    }
}

fn csv_row(fields: &[&str]) -> String {
    let mut row = fields
        .iter()
        .map(|field| csv_escape(field))
        .collect::<Vec<_>>()
        .join(",");
    row.push('\n');
    row
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    fn temp_base(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("medical_inventory_{name}_{nanos}"))
    }

    fn cleanup(path: &Path) {
        if path.starts_with(std::env::temp_dir()) {
            let _ = fs::remove_dir_all(path);
        }
    }

    fn data_with_saved_at(saved_at: &str) -> AppData {
        let mut data = AppData::default();
        data.saved_at = saved_at.into();
        data
    }

    fn sample_product() -> Product {
        Product {
            id: Uuid::new_v4(),
            name: "测试产品".into(),
            purchase_price: 60.0,
            outbound_price: 100.0,
            specification: "规格A".into(),
            brand: "测试品牌".into(),
            image_path: String::new(),
            description: "含,逗号".into(),
            created_at: "2026-05-30 00:00:00".into(),
        }
    }

    fn sample_order() -> Order {
        Order {
            id: Uuid::new_v4(),
            product_id: None,
            product_name: "测试产品".into(),
            item_no: "ITEM-001".into(),
            order_time: "2026-05-30".into(),
            delivery_date: "2026-05-31".into(),
            customer_unit: "测试医院".into(),
            customer_name: "张三".into(),
            brand: "测试品牌".into(),
            unit: "件".into(),
            catalog_price: 120.0,
            quantity: 3.0,
            invoice_total: 300.0,
            invoice_status: String::new(),
            is_shipped: true,
            cashback: 0.0,
            cost_discount: 0.0,
            cost_unit_price: 60.0,
            cost_total: 180.0,
            sale_discount: 0.0,
            invoice_unit_price: 100.0,
            gross_profit: 120.0,
            remark: "测试备注".into(),
            invoice_no: String::new(),
            is_paid: true,
            paid_time: String::new(),
            returned_quantity: 1.0,
            created_at: "2026-05-30 00:00:00".into(),
        }
    }

    #[test]
    fn save_writes_data_and_creates_backup_when_replacing() {
        let base = temp_base("save");
        let storage = Storage::from_base_dir(base.clone());

        storage.save(&data_with_saved_at("first")).unwrap();
        storage.save(&data_with_saved_at("second")).unwrap();

        let saved = fs::read_to_string(storage.data_file()).unwrap();
        let saved_data = serde_json::from_str::<AppData>(&saved).unwrap();
        assert_eq!(saved_data.saved_at, "second");

        let backup_count = fs::read_dir(storage.backup_dir())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("inventory-")
            })
            .count();
        assert_eq!(backup_count, 1);

        cleanup(&base);
    }

    #[test]
    fn load_recovers_corrupt_data_file_from_backup_and_writes_it_back() {
        let base = temp_base("recover");
        let storage = Storage::from_base_dir(base.clone());
        let data = data_with_saved_at("recoverable");

        storage.create_manual_backup(&data).unwrap();
        storage.ensure_dirs().unwrap();
        fs::write(storage.data_file(), "{broken json").unwrap();

        let outcome = storage.load();

        assert_eq!(outcome.data.saved_at, "recoverable");
        assert!(outcome.notice.contains("写回主数据文件"));

        let restored = fs::read_to_string(storage.data_file()).unwrap();
        let restored_data = serde_json::from_str::<AppData>(&restored).unwrap();
        assert_eq!(restored_data.saved_at, "recoverable");

        cleanup(&base);
    }

    #[test]
    fn create_outbound_note_writes_readable_order_summary() {
        let base = temp_base("outbound");
        let storage = Storage::from_base_dir(base.clone());

        let note_file = storage.create_outbound_note(&sample_order()).unwrap();
        let content = fs::read_to_string(note_file).unwrap();

        assert!(content.contains("医疗产品出库单"));
        assert!(content.contains("产品名称: 测试产品"));
        assert!(content.contains("货号: ITEM-001"));
        assert!(content.contains("当前出库数量: 2.00 件"));

        cleanup(&base);
    }

    #[test]
    fn csv_escape_quotes_commas_quotes_and_newlines() {
        assert_eq!(csv_escape("普通文本"), "普通文本");
        assert_eq!(csv_escape("甲,乙"), "\"甲,乙\"");
        assert_eq!(csv_escape("他说\"好\""), "\"他说\"\"好\"\"\"");
        assert_eq!(csv_escape("第一行\n第二行"), "\"第一行\n第二行\"");
    }

    #[test]
    fn export_orders_csv_writes_excel_friendly_table() {
        let base = temp_base("orders_csv");
        let storage = Storage::from_base_dir(base.clone());

        let csv_file = storage.export_orders_csv(&[sample_order()]).unwrap();
        let content = fs::read_to_string(csv_file).unwrap();

        assert!(content.starts_with('\u{feff}'));
        assert!(content.contains("产品名称,货号,订购时间"));
        assert!(content.contains("测试产品,ITEM-001"));
        assert!(content.contains("销售总价"));

        cleanup(&base);
    }

    #[test]
    fn export_products_csv_writes_product_table() {
        let base = temp_base("products_csv");
        let storage = Storage::from_base_dir(base.clone());

        let csv_file = storage.export_products_csv(&[sample_product()]).unwrap();
        let content = fs::read_to_string(csv_file).unwrap();

        assert!(content.contains("产品名称,进货金额,出库金额"));
        assert!(content.contains("测试产品,60.00,100.00"));
        assert!(content.contains("\"含,逗号\""));

        cleanup(&base);
    }

    #[test]
    fn read_products_csv_parses_excel_friendly_product_rows() {
        let base = temp_base("read_products_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let csv_file = base.join("products.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                PRODUCT_CSV_HEADERS,
                &[csv_row(&[
                    "导入产品",
                    "¥1,200.50",
                    "￥1，500.00",
                    "规格A",
                    "品牌A",
                    "",
                    "含,逗号",
                    "",
                ])],
            ),
        )
        .unwrap();

        let products = storage.read_products_csv(&csv_file).unwrap();

        assert_eq!(products.len(), 1);
        assert_eq!(products[0].name, "导入产品");
        assert_eq!(products[0].purchase_price, 1200.50);
        assert_eq!(products[0].outbound_price, 1500.00);
        assert_eq!(products[0].description, "含,逗号");
        assert!(!products[0].created_at.is_empty());

        cleanup(&base);
    }

    #[test]
    fn read_products_csv_defaults_blank_prices_to_zero() {
        let base = temp_base("read_products_blank_prices_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let csv_file = base.join("products.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                PRODUCT_CSV_HEADERS,
                &[csv_row(&["空价格产品", "", "", "", "", "", "", ""])],
            ),
        )
        .unwrap();

        let products = storage.read_products_csv(&csv_file).unwrap();

        assert_eq!(products[0].purchase_price, 0.0);
        assert_eq!(products[0].outbound_price, 0.0);

        cleanup(&base);
    }

    #[test]
    fn read_orders_csv_parses_excel_friendly_order_rows() {
        let base = temp_base("read_orders_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let product = sample_product();
        let csv_file = base.join("orders.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                ORDER_CSV_HEADERS,
                &[csv_row(&[
                    &product.name,
                    "ITEM-001",
                    "2026/5/30",
                    "2026.5.31",
                    "测试医院",
                    "张三",
                    &product.brand,
                    "件",
                    "¥120.00",
                    "3",
                    "360",
                    "0",
                    "0",
                    "60",
                    "180",
                    "0",
                    "120",
                    "180",
                    "测试备注",
                    "1",
                    "2026-05-30 12:00:00",
                ])],
            ),
        )
        .unwrap();

        let orders = storage
            .read_orders_csv(&csv_file, &[product.clone()])
            .unwrap();

        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].product_name, product.name);
        assert_eq!(orders[0].product_id, Some(product.id));
        assert_eq!(orders[0].order_time, "2026-05-30");
        assert_eq!(orders[0].delivery_date, "2026-05-31");
        assert_eq!(orders[0].returned_quantity, 1.0);
        assert_eq!(orders[0].created_at, "2026-05-30 12:00:00");

        cleanup(&base);
    }

    #[test]
    fn read_orders_csv_rejects_non_positive_quantity() {
        let base = temp_base("read_orders_bad_quantity_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let csv_file = base.join("orders.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                ORDER_CSV_HEADERS,
                &[csv_row(&[
                    "测试产品",
                    "ITEM-001",
                    "2026-05-30",
                    "2026-05-31",
                    "测试医院",
                    "张三",
                    "测试品牌",
                    "件",
                    "",
                    "0",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])],
            ),
        )
        .unwrap();

        let err = storage.read_orders_csv(&csv_file, &[]).unwrap_err();

        assert!(err.contains("数量必须大于 0"));

        cleanup(&base);
    }

    #[test]
    fn read_returns_csv_parses_and_links_return_rows() {
        let base = temp_base("read_returns_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let product = sample_product();
        let order = sample_order();
        let csv_file = base.join("returns.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                RETURN_CSV_HEADERS,
                &[csv_row(&[
                    &product.name,
                    "ITEM-001",
                    "2026/6/1",
                    "测试医院",
                    "张三",
                    &product.brand,
                    "件",
                    "¥120.00",
                    "1",
                    "120",
                    "0",
                    "0",
                    "60",
                    "60",
                    "0",
                    "120",
                    "60",
                    "退货备注",
                    &order.id.to_string(),
                    "2026-06-01 12:00:00",
                ])],
            ),
        )
        .unwrap();

        let returns = storage
            .read_returns_csv(&csv_file, &[order.clone()], &[product.clone()])
            .unwrap();

        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0].source_order_id, order.id);
        assert_eq!(returns[0].product_id, Some(product.id));
        assert_eq!(returns[0].return_time, "2026-06-01");
        assert_eq!(returns[0].quantity, 1.0);
        assert_eq!(returns[0].remark, "退货备注");

        cleanup(&base);
    }

    #[test]
    fn read_returns_csv_rejects_non_positive_quantity() {
        let base = temp_base("read_returns_bad_quantity_csv");
        let storage = Storage::from_base_dir(base.clone());
        storage.ensure_dirs().unwrap();
        let order = sample_order();
        let csv_file = base.join("returns.csv");
        fs::write(
            &csv_file,
            csv_with_header(
                RETURN_CSV_HEADERS,
                &[csv_row(&[
                    "测试产品",
                    "ITEM-001",
                    "2026-06-01",
                    "测试医院",
                    "张三",
                    "测试品牌",
                    "件",
                    "",
                    "-1",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    &order.id.to_string(),
                    "",
                ])],
            ),
        )
        .unwrap();

        let err = storage
            .read_returns_csv(&csv_file, &[order], &[])
            .unwrap_err();

        assert!(err.contains("数量必须大于 0"));

        cleanup(&base);
    }

    #[test]
    fn export_all_csv_writes_three_tables() {
        let base = temp_base("all_csv");
        let storage = Storage::from_base_dir(base.clone());
        let mut data = AppData::default();
        data.products.push(sample_product());
        data.orders.push(sample_order());

        let paths = storage.export_all_csv(&data).unwrap();

        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|path| path.exists()));

        cleanup(&base);
    }

    #[test]
    fn export_csv_templates_writes_header_only_files() {
        let base = temp_base("csv_templates");
        let storage = Storage::from_base_dir(base.clone());

        let paths = storage.export_csv_templates().unwrap();

        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|path| path.exists()));

        let products_template = paths
            .iter()
            .find(|path| path.to_string_lossy().contains("template-products"))
            .expect("products template should be exported");
        let content = fs::read_to_string(products_template).unwrap();

        assert!(content.starts_with('\u{feff}'));
        assert_eq!(content.lines().count(), 1);
        assert!(content.contains("产品名称,进货金额,出库金额"));

        cleanup(&base);
    }
}

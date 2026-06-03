import type { AppData, Invoice, Order, Product, ReturnRecord } from './types'

export const PRODUCT_HEADERS = [
  '产品名称',
  '进货金额',
  '出库金额',
  '规格/型号',
  '品牌',
  '单位',
  '描述',
  '创建时间',
]

export const ORDER_HEADERS = [
  '月份',
  '订购时间',
  '订货单位',
  '订货人',
  '品牌',
  '货号',
  '品名',
  '单位',
  '目录价',
  '数量',
  '开票总价',
  '开票情况',
  '是否发货',
  '返现',
  '成本折扣',
  '成本单价',
  '成本总价',
  '售价折扣',
  '开票单价',
  '毛利',
  '备注',
  '发票号',
  '是否回款',
  '回款时间',
]

export const INVOICE_HEADERS = [
  '发票号',
  '开票日期',
  '开票总额',
  '是否回款',
  '回款时间',
  '关联出库单',
  '备注',
  '创建时间',
]

export const RETURN_HEADERS = [
  '产品名称',
  '货号',
  '退货日期',
  '订货单位',
  '订货人',
  '品牌',
  '单位',
  '目录价',
  '数量',
  '销售总价',
  '返现',
  '成本折扣',
  '成本单价',
  '成本总价',
  '售价折扣',
  '销售单价',
  '毛利',
  '备注',
  '关联订单ID',
  '创建时间',
]

export function defaultData(): AppData {
  return {
    version: 1,
    savedAt: nowString(),
    products: [],
    customers: [],
    orders: [],
    invoices: [],
    returns: [],
  }
}

export function createId(): string {
  return crypto.randomUUID()
}

export function todayString(): string {
  return formatLocalDate(new Date())
}

export function formatLocalDate(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

export function nowString(): string {
  const date = new Date()
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

export function nowCompactString(): string {
  const date = new Date()
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`
}

export function formatMoney(value: number): string {
  return Number.isFinite(value) ? value.toFixed(2) : '0.00'
}

export function parseAmount(input: string): number {
  const text = input.trim().replace(/[,\uFF0C\u00A5\uFFE5]/g, '')
  if (!text) {
    return 0
  }
  const percent = text.endsWith('%')
  const numberText = percent ? text.slice(0, -1).trim() : text
  const value = Number(numberText)
  if (!Number.isFinite(value)) {
    throw new Error(`数值格式不正确: ${input}`)
  }
  return percent ? value / 100 : value
}

export function normalizeDate(input: string): string {
  const text = input.trim()
  if (!text) {
    return ''
  }
  const match = text.match(/^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})$/)
  if (!match) {
    return text
  }
  const [, year, month, day] = match
  return `${year}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`
}

export function dateValue(input: string): number {
  const normalized = normalizeDate(input)
  const time = Date.parse(`${normalized}T00:00:00`)
  return Number.isFinite(time) ? time : 0
}

export function currentOutboundQuantity(order: Order): number {
  return Math.max(order.quantity - order.returnedQuantity, 0)
}

export function proratedAmount(total: number, originalQuantity: number, quantity: number): number {
  if (originalQuantity <= 0) {
    return 0
  }
  return total * (quantity / originalQuantity)
}

export function currentOutboundAmount(order: Order): number {
  return proratedAmount(order.invoiceTotal, order.quantity, currentOutboundQuantity(order))
}

export function csvRow(fields: Array<string | number>): string {
  return fields.map((field) => csvEscape(String(field ?? ''))).join(',') + '\n'
}

export function csvWithHeader(headers: string[], rows: string[]): string {
  return `\uFEFF${csvRow(headers)}${rows.join('')}`
}

export function csvEscape(value: string): string {
  if (/[",\n\r]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

export function parseCsv(content: string): Record<string, string>[] {
  const rows: string[][] = []
  let row: string[] = []
  let field = ''
  let quoted = false

  for (let index = 0; index < content.length; index += 1) {
    const char = content[index]
    const next = content[index + 1]

    if (quoted) {
      if (char === '"' && next === '"') {
        field += '"'
        index += 1
      } else if (char === '"') {
        quoted = false
      } else {
        field += char
      }
    } else if (char === '"') {
      quoted = true
    } else if (char === ',') {
      row.push(field)
      field = ''
    } else if (char === '\n') {
      row.push(field)
      rows.push(row)
      row = []
      field = ''
    } else if (char !== '\r') {
      field += char
    }
  }

  if (field || row.length > 0) {
    row.push(field)
    rows.push(row)
  }

  if (rows.length === 0) {
    return []
  }

  const headers = rows[0].map((header) => header.trim().replace(/^\uFEFF/, ''))
  return rows
    .slice(1)
    .filter((item) => item.some((fieldValue) => fieldValue.trim()))
    .map((item) =>
      Object.fromEntries(headers.map((header, index) => [header, item[index]?.trim() ?? ''])),
    )
}

export function downloadText(filename: string, content: string, type = 'text/csv;charset=utf-8'): void {
  const blob = new Blob([content], { type })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
}

export function exportProductsCsv(products: Product[]): string {
  const rows = products.map((product) =>
    csvRow([
      product.name,
      formatMoney(product.purchasePrice),
      formatMoney(product.outboundPrice),
      product.specification,
      product.brand,
      product.unit || '件',
      product.description,
      product.createdAt,
    ]),
  )
  return csvWithHeader(PRODUCT_HEADERS, rows)
}

export function exportOrdersCsv(orders: Order[]): string {
  const rows = orders.map((order) =>
    csvRow([
      order.month || normalizeDate(order.orderTime).slice(0, 7),
      order.orderTime,
      order.customerUnit,
      order.customerName,
      order.brand,
      order.itemNo,
      order.productName,
      order.unit,
      formatMoney(order.catalogPrice),
      formatMoney(order.quantity),
      formatMoney(order.invoiceTotal),
      order.invoiceStatus,
      order.isShipped ? '是' : '否',
      formatMoney(order.cashback),
      formatMoney(order.costDiscount),
      formatMoney(order.costUnitPrice),
      formatMoney(order.costTotal),
      formatMoney(order.saleDiscount),
      formatMoney(order.invoiceUnitPrice),
      formatMoney(order.grossProfit),
      order.remark,
      order.invoiceNo,
      order.isPaid ? '是' : '否',
      order.paidTime,
    ]),
  )
  return csvWithHeader(ORDER_HEADERS, rows)
}

export function exportInvoicesCsv(invoices: Invoice[]): string {
  const rows = invoices.map((invoice) =>
    csvRow([
      invoice.invoiceNo,
      invoice.invoiceDate,
      formatMoney(invoice.totalAmount),
      invoice.isPaid ? '是' : '否',
      invoice.paidTime,
      invoice.lines.map((line) => `${line.orderNo}:${formatMoney(line.invoiceAmount)}`).join('; '),
      invoice.remark,
      invoice.createdAt,
    ]),
  )
  return csvWithHeader(INVOICE_HEADERS, rows)
}

export function exportReturnsCsv(records: ReturnRecord[]): string {
  const rows = records.map((record) =>
    csvRow([
      record.productName,
      record.itemNo,
      record.returnTime,
      record.customerUnit,
      record.customerName,
      record.brand,
      record.unit,
      formatMoney(record.catalogPrice),
      formatMoney(record.quantity),
      formatMoney(record.invoiceTotal),
      formatMoney(record.cashback),
      formatMoney(record.costDiscount),
      formatMoney(record.costUnitPrice),
      formatMoney(record.costTotal),
      formatMoney(record.saleDiscount),
      formatMoney(record.invoiceUnitPrice),
      formatMoney(record.grossProfit),
      record.remark,
      record.sourceOrderId,
      record.createdAt,
    ]),
  )
  return csvWithHeader(RETURN_HEADERS, rows)
}

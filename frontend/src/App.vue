<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import { ElMessageBox } from 'element-plus/es/components/message-box/index'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import * as XLSX from 'xlsx'
import {
  Box,
  DataAnalysis,
  DocumentAdd,
  Download,
  Goods,
  Moon,
  Plus,
  Printer,
  Refresh,
  Search,
  Setting,
  Sunny,
  Tickets,
  Upload,
  User,
} from '@element-plus/icons-vue'
import type { AppData, Customer, CustomerForm, DailyStat, Invoice, InvoiceLine, Order, OrderForm, Product, ProductForm, ReturnForm, ReturnRecord } from './types'
import {
  INVOICE_HEADERS,
  ORDER_HEADERS,
  PRODUCT_HEADERS,
  RETURN_HEADERS,
  createId,
  currentOutboundAmount,
  currentOutboundQuantity,
  dateValue,
  defaultData,
  downloadText,
  formatLocalDate,
  formatMoney,
  normalizeDate,
  nowCompactString,
  nowString,
  parseAmount,
  proratedAmount,
  todayString,
} from './inventory'

type ImportType = 'products' | 'orders' | 'invoices' | 'returns'
type AmountSummaryRow = Pick<Order, 'quantity' | 'invoiceTotal' | 'cashback' | 'costTotal' | 'grossProfit'>
type PrintProfileKey = 'dotMatrix' | 'a4Portrait'

interface InvoiceForm {
  invoiceNo: string
  invoiceDate: string
  isPaid: boolean
  paidTime: string
  remark: string
  lines: InvoiceLine[]
}

interface AmountSummary {
  quantity: number
  invoiceTotal: number
  cashback: number
  costTotal: number
  grossProfit: number
  averageInvoiceUnitPrice: number
  averageCostUnitPrice: number
}

interface ChartPoint {
  x: number
  y: number
  value: number
  label: string
}

interface ChartTick {
  value: number
  y: number
}

interface ChartLabel {
  label: string
  x: number
}

interface PieSlice {
  label: string
  value: number
  percent: number
  className: string
  dashArray: string
  dashOffset: number
}

interface DashboardDailyStat extends DailyStat {
  netSalesAmount: number
  grossProfitAmount: number
}

interface BatchOrderLine {
  id: string
  productId: string | null
  productName: string
  itemNo: string
  brand: string
  unit: string
  quantity: string
  invoiceUnitPrice: string
  costUnitPrice: string
  invoiceTotal: string
  costTotal: string
  cashback: string
  grossProfit: string
  remark: string
}

interface PrintProfile {
  key: PrintProfileKey
  label: string
  pageSize: string
  margin: string
  sheetWidth: string
  sheetMinHeight: string
  titleSize: string
  subtitleSize: string
  orderNoSize: string
  tableFontSize: string
  cellHeight: string
  itemRowHeight: string
  blankRowHeight: string
  signatureHeight: string
  blankRows: number
}

interface PrintContentSettings {
  companyName: string
  documentTitle: string
  orderNoPrefix: string
  warehouseName: string
  receiverSignatureLabel: string
  receiverSignatureHint: string
}

const STORAGE_KEY = 'medical-inventory-vue-data'
const THEME_KEY = 'medical-inventory-theme'
const PRINT_PROFILE_STORAGE_KEY = 'medical-inventory-print-profile'
const PRINT_CONTENT_SETTINGS_STORAGE_KEY = 'medical-inventory-print-content-settings'
const AUTO_BACKUP_STORAGE_KEY = 'medical-inventory-auto-backup'
const AUTO_BACKUP_FILENAME = 'inventory-auto-backup.json'
const AUTO_BACKUP_INTERVAL = 10 * 60 * 1000
const CHART_WIDTH = 760
const CHART_HEIGHT = 280
const CHART_PADDING = {
  top: 28,
  right: 34,
  bottom: 42,
  left: 78,
}
const PIE_RADIUS = 72
const PIE_CIRCUMFERENCE = 2 * Math.PI * PIE_RADIUS
const DEFAULT_PRINT_CONTENT_SETTINGS: PrintContentSettings = {
  companyName: '武汉维优诺生物科技有限公司',
  documentTitle: '送（销）货单',
  orderNoPrefix: 'No:',
  warehouseName: '普通舱',
  receiverSignatureLabel: '收货人签名:',
  receiverSignatureHint: '（“货物”“发票”已收到）',
}

const PRINT_PROFILES: PrintProfile[] = [
  {
    key: 'dotMatrix',
    label: '针式打印机',
    pageSize: '241mm 140mm',
    margin: '4mm',
    sheetWidth: '233mm',
    sheetMinHeight: '132mm',
    titleSize: '13.5pt',
    subtitleSize: '12pt',
    orderNoSize: '10.5pt',
    tableFontSize: '9.6pt',
    cellHeight: '7.8mm',
    itemRowHeight: '10.5mm',
    blankRowHeight: '7.6mm',
    signatureHeight: '13mm',
    blankRows: 3,
  },
  {
    key: 'a4Portrait',
    label: 'A4 纵版',
    pageSize: 'A4 portrait',
    margin: '6mm',
    sheetWidth: '198mm',
    sheetMinHeight: '285mm',
    titleSize: '16pt',
    subtitleSize: '14pt',
    orderNoSize: '11.5pt',
    tableFontSize: '10.5pt',
    cellHeight: '10mm',
    itemRowHeight: '12mm',
    blankRowHeight: '10mm',
    signatureHeight: '18mm',
    blankRows: 12,
  },
]

const activePage = ref('dashboard')
const themeMode = ref<'light' | 'dark'>(localStorage.getItem(THEME_KEY) === 'dark' ? 'dark' : 'light')
const printProfileKey = ref<PrintProfileKey>(normalizePrintProfileKey(localStorage.getItem(PRINT_PROFILE_STORAGE_KEY)))
const printContentSettings = reactive<PrintContentSettings>(loadPrintContentSettings())
const statusMessage = ref('就绪')
const productQuery = ref('')
const customerQuery = ref('')
const orderQuery = ref('')
const invoiceQuery = ref('')
const returnQuery = ref('')
const orderDateRange = ref<[string, string] | []>([])
const invoiceDateRange = ref<[string, string] | []>([])
const returnDateRange = ref<[string, string] | []>([])
const productPage = ref(1)
const productPageSize = ref(8)
const customerPage = ref(1)
const customerPageSize = ref(8)
const orderPage = ref(1)
const orderPageSize = ref(8)
const invoicePage = ref(1)
const invoicePageSize = ref(8)
const returnPage = ref(1)
const returnPageSize = ref(8)
const viewportHeight = ref(window.innerHeight)
const tableHeight = ref(520)
let tableHeightFrame = 0
let autoBackupTimer: ReturnType<typeof setInterval> | null = null
const editingProductId = ref<string | null>(null)
const editingCustomerId = ref<string | null>(null)
const editingOrderId = ref<string | null>(null)
const editingInvoiceId = ref<string | null>(null)
const editingReturnId = ref<string | null>(null)
const productDialogVisible = ref(false)
const customerDialogVisible = ref(false)
const orderDialogVisible = ref(false)
const invoiceDialogVisible = ref(false)
const returnDialogVisible = ref(false)
const printSettingsDialogVisible = ref(false)
const isPrinting = ref(false)
const batchOrderDialogVisible = ref(false)
const productFileInput = ref<HTMLInputElement | null>(null)
const orderFileInput = ref<HTMLInputElement | null>(null)
const returnFileInput = ref<HTMLInputElement | null>(null)
const backupFileInput = ref<HTMLInputElement | null>(null)
const selectedOrders = ref<Order[]>([])
const invoiceForm = reactive<InvoiceForm>(blankInvoiceForm())
const importDialogVisible = ref(false)
const activeImportType = ref<ImportType>('products')
const batchOrderCustomerId = ref<string | null>(null)
const batchOrderTime = ref(todayString())
const batchDeliveryDate = ref(todayString())
const batchPaidTime = ref(todayString())
const batchOrderRows = ref<BatchOrderLine[]>([])

const dashboardFilter = reactive({
  startDate: addDays(todayString(), -6),
  endDate: todayString(),
})

const data = reactive<AppData>(loadData())
const productForm = reactive<ProductForm>(blankProductForm())
const customerForm = reactive<CustomerForm>(blankCustomerForm())
const orderForm = reactive<OrderForm>(blankOrderForm())
const returnForm = reactive<ReturnForm>(blankReturnForm())

watch(
  data,
  (value) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
  },
  { deep: true },
)

watch(
  themeMode,
  (mode) => {
    document.documentElement.dataset.theme = mode
    document.documentElement.classList.toggle('dark', mode === 'dark')
    localStorage.setItem(THEME_KEY, mode)
  },
  { immediate: true },
)

watch(printProfileKey, (key) => {
  localStorage.setItem(PRINT_PROFILE_STORAGE_KEY, key)
})

watch(
  printContentSettings,
  (settings) => {
    localStorage.setItem(PRINT_CONTENT_SETTINGS_STORAGE_KEY, JSON.stringify(settings))
  },
  { deep: true },
)

const isDarkMode = computed(() => themeMode.value === 'dark')
const autoPageSize = computed(() => Math.max(5, Math.floor((tableHeight.value - 54) / 46)))
const activePrintProfile = computed(() => PRINT_PROFILES.find((profile) => profile.key === printProfileKey.value) ?? PRINT_PROFILES[0])

onMounted(() => {
  window.addEventListener('resize', updateViewportHeight)
  scheduleTableHeightUpdate()
  void writeAutoBackup()
  autoBackupTimer = setInterval(() => {
    void writeAutoBackup()
  }, AUTO_BACKUP_INTERVAL)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', updateViewportHeight)
  cancelAnimationFrame(tableHeightFrame)
  if (autoBackupTimer) {
    clearInterval(autoBackupTimer)
    autoBackupTimer = null
  }
})

watch(productQuery, () => {
  productPage.value = 1
})

watch(customerQuery, () => {
  customerPage.value = 1
})

watch(orderQuery, () => {
  orderPage.value = 1
})

watch(invoiceQuery, () => {
  invoicePage.value = 1
})

watch(orderDateRange, () => {
  orderPage.value = 1
})

watch(invoiceDateRange, () => {
  invoicePage.value = 1
})

watch(returnQuery, () => {
  returnPage.value = 1
})

watch(returnDateRange, () => {
  returnPage.value = 1
})

watch(
  () => [orderForm.quantity, orderForm.invoiceUnitPrice, orderForm.costUnitPrice, orderForm.cashback],
  () => {
    syncOrderAmounts()
  },
)

watch(
  () => orderForm.orderTime,
  (value) => {
    orderForm.month = monthFromDate(value)
    if (!orderForm.deliveryDate) {
      orderForm.deliveryDate = normalizeDate(value) || todayString()
    }
  },
)

watch(
  () => [returnForm.quantity, returnForm.invoiceUnitPrice, returnForm.costUnitPrice, returnForm.cashback],
  () => {
    syncReturnAmounts()
  },
)

watch(
  autoPageSize,
  (pageSize) => {
    productPageSize.value = pageSize
    customerPageSize.value = pageSize
    orderPageSize.value = pageSize
    invoicePageSize.value = pageSize
    returnPageSize.value = pageSize
  },
  { immediate: true },
)

watch([activePage, productQuery, customerQuery, orderQuery, invoiceQuery, returnQuery, orderDateRange, invoiceDateRange, returnDateRange], () => {
  scheduleTableHeightUpdate()
})

const navItems = [
  { key: 'dashboard', label: '看板', icon: DataAnalysis },
  { key: 'products', label: '产品管理', icon: Goods },
  { key: 'customers', label: '客户管理', icon: User },
  { key: 'orders', label: '出库单', icon: Tickets },
  { key: 'invoices', label: '发票管理', icon: DocumentAdd },
  { key: 'returns', label: '退货管理', icon: Refresh },
]

const currentNavLabel = computed(() => navItems.find((item) => item.key === activePage.value)?.label ?? '看板')
const activeImportLabel = computed(() => importTypeLabel(activeImportType.value))

const sortedProducts = computed(() =>
  [...data.products].sort((left, right) =>
    identity(left.name)
      .localeCompare(identity(right.name), 'zh-CN')
      || identity(left.brand).localeCompare(identity(right.brand), 'zh-CN')
      || identity(left.specification).localeCompare(identity(right.specification), 'zh-CN'),
  ),
)

const filteredProducts = computed(() =>
  sortedProducts.value.filter((product) =>
    matchesQuery(productQuery.value, [
      product.name,
      product.brand,
      product.specification,
      product.description,
    ]),
  ),
)

const pagedProducts = computed(() => paginate(filteredProducts.value, productPage.value, productPageSize.value))

const sortedCustomers = computed(() =>
  [...data.customers].sort(
    (left, right) =>
      identity(left.unit).localeCompare(identity(right.unit), 'zh-CN')
      || identity(left.contactName).localeCompare(identity(right.contactName), 'zh-CN'),
  ),
)

const filteredCustomers = computed(() =>
  sortedCustomers.value.filter((customer) =>
    matchesQuery(customerQuery.value, [
      customer.unit,
      customer.contactName,
      customer.phone,
      customer.address,
      customer.remark,
    ]),
  ),
)

const pagedCustomers = computed(() => paginate(filteredCustomers.value, customerPage.value, customerPageSize.value))
const batchOrderCustomer = computed(() => data.customers.find((customer) => customer.id === batchOrderCustomerId.value) ?? null)

const sortedOrders = computed(() =>
  [...data.orders].sort(
    (left, right) => dateValue(right.orderTime) - dateValue(left.orderTime) || right.createdAt.localeCompare(left.createdAt),
  ),
)

const filteredOrders = computed(() =>
  sortedOrders.value.filter((order) => {
    const queryMatched = matchesQuery(orderQuery.value, [
      order.orderNo,
      order.productName,
      order.itemNo,
      order.customerUnit,
      order.customerName,
      order.invoiceNo,
      order.remark,
    ])
    return queryMatched && dateInRange(order.orderTime, orderDateRange.value)
  }),
)

const pagedOrders = computed(() => paginate(filteredOrders.value, orderPage.value, orderPageSize.value))
const filteredOrderAmountSummary = computed(() => summarizeAmounts(filteredOrders.value))

const sortedInvoices = computed(() =>
  [...data.invoices].sort(
    (left, right) => dateValue(right.invoiceDate) - dateValue(left.invoiceDate) || right.createdAt.localeCompare(left.createdAt),
  ),
)

const filteredInvoices = computed(() =>
  sortedInvoices.value.filter((invoice) => {
    const queryMatched = matchesQuery(invoiceQuery.value, [
      invoice.invoiceNo,
      invoice.remark,
      invoice.lines.map((line) => `${line.orderNo} ${line.productName} ${line.itemNo} ${line.customerUnit}`).join(' '),
    ])
    return queryMatched && dateInRange(invoice.invoiceDate, invoiceDateRange.value)
  }),
)

const pagedInvoices = computed(() => paginate(filteredInvoices.value, invoicePage.value, invoicePageSize.value))
const invoiceTotalAmount = computed(() => data.invoices.reduce((sum, invoice) => sum + invoice.totalAmount, 0))
const filteredInvoiceTotalAmount = computed(() => filteredInvoices.value.reduce((sum, invoice) => sum + invoice.totalAmount, 0))
const invoiceFormTotalAmount = computed(() => invoiceForm.lines.reduce((sum, line) => sum + Number(line.invoiceAmount || 0), 0))

const sortedReturns = computed(() =>
  [...data.returns].sort(
    (left, right) => dateValue(right.returnTime) - dateValue(left.returnTime) || right.createdAt.localeCompare(left.createdAt),
  ),
)

const filteredReturns = computed(() =>
  sortedReturns.value.filter((record) => {
    const queryMatched = matchesQuery(returnQuery.value, [
      record.productName,
      record.itemNo,
      record.customerUnit,
      record.customerName,
      record.remark,
    ])
    return queryMatched && dateInRange(record.returnTime, returnDateRange.value)
  }),
)

const pagedReturns = computed(() => paginate(filteredReturns.value, returnPage.value, returnPageSize.value))
const filteredReturnAmountSummary = computed(() => summarizeAmounts(filteredReturns.value))

const returnableOrders = computed(() => data.orders.filter((order) => currentOutboundQuantity(order) > 0))

const totalOrderAmount = computed(() => data.orders.reduce((sum, order) => sum + order.invoiceTotal, 0))
const totalReturnAmount = computed(() => data.returns.reduce((sum, record) => sum + record.invoiceTotal, 0))
const netAmount = computed(() => totalOrderAmount.value - totalReturnAmount.value)
const totalOrderGrossProfit = computed(() => data.orders.reduce((sum, order) => sum + order.grossProfit, 0))
const totalReturnGrossProfit = computed(() => data.returns.reduce((sum, record) => sum + record.grossProfit, 0))
const netGrossProfit = computed(() => totalOrderGrossProfit.value - totalReturnGrossProfit.value)
const currentOutboundTotal = computed(() => data.orders.reduce((sum, order) => sum + currentOutboundAmount(order), 0))
const returnableOrderCount = computed(() => data.orders.filter((order) => currentOutboundQuantity(order) > 0).length)
const todayOutboundAmount = computed(() =>
  data.orders
    .filter((order) => normalizeDate(order.deliveryDate) === todayString())
    .reduce((sum, order) => sum + order.invoiceTotal, 0),
)
const todayReturnAmount = computed(() =>
  data.returns
    .filter((record) => normalizeDate(record.returnTime) === todayString())
    .reduce((sum, record) => sum + record.invoiceTotal, 0),
)
const todayOrderGrossProfit = computed(() =>
  data.orders
    .filter((order) => normalizeDate(order.deliveryDate) === todayString())
    .reduce((sum, order) => sum + order.grossProfit, 0),
)
const todayReturnGrossProfit = computed(() =>
  data.returns
    .filter((record) => normalizeDate(record.returnTime) === todayString())
    .reduce((sum, record) => sum + record.grossProfit, 0),
)
const todayNetAmount = computed(() => todayOutboundAmount.value - todayReturnAmount.value)
const todayNetGrossProfit = computed(() => todayOrderGrossProfit.value - todayReturnGrossProfit.value)
const netGrossProfitRate = computed(() => percentOf(netGrossProfit.value, netAmount.value))
const todayNetGrossProfitRate = computed(() => percentOf(todayNetGrossProfit.value, todayNetAmount.value))

const dailySeries = computed<DashboardDailyStat[]>(() => {
  const start = dateValue(dashboardFilter.startDate)
  const end = dateValue(dashboardFilter.endDate)
  if (!start || !end || start > end) {
    return []
  }

  const result: DashboardDailyStat[] = []
  const cursor = new Date(start)
  const last = new Date(end)
  while (cursor <= last && result.length < 45) {
    const label = formatLocalDate(cursor)
    const outboundAmount = data.orders
      .filter((order) => normalizeDate(order.deliveryDate) === label)
      .reduce((sum, order) => sum + order.invoiceTotal, 0)
    const returnAmount = data.returns
      .filter((record) => normalizeDate(record.returnTime) === label)
      .reduce((sum, record) => sum + record.invoiceTotal, 0)
    const orderGrossProfit = data.orders
      .filter((order) => normalizeDate(order.deliveryDate) === label)
      .reduce((sum, order) => sum + order.grossProfit, 0)
    const returnGrossProfit = data.returns
      .filter((record) => normalizeDate(record.returnTime) === label)
      .reduce((sum, record) => sum + record.grossProfit, 0)
    result.push({
      label,
      outboundAmount,
      returnAmount,
      netSalesAmount: outboundAmount - returnAmount,
      grossProfitAmount: orderGrossProfit - returnGrossProfit,
    })
    cursor.setDate(cursor.getDate() + 1)
  }
  return result
})

const maxDailyAmount = computed(() =>
  Math.max(1, ...dailySeries.value.flatMap((item) => [item.netSalesAmount, item.grossProfitAmount].map(Math.abs))),
)

const chartInnerWidth = CHART_WIDTH - CHART_PADDING.left - CHART_PADDING.right
const chartInnerHeight = CHART_HEIGHT - CHART_PADDING.top - CHART_PADDING.bottom

const chartMaxAmount = computed(() => {
  const rawMax = maxDailyAmount.value
  if (rawMax <= 1) {
    return 1
  }
  const magnitude = 10 ** Math.max(0, Math.floor(Math.log10(rawMax)) - 1)
  return Math.ceil(rawMax / magnitude) * magnitude
})

const chartScaleMax = computed(() => chartMaxAmount.value)
const chartScaleMin = computed(() =>
  dailySeries.value.some((item) => item.netSalesAmount < 0 || item.grossProfitAmount < 0) ? -chartMaxAmount.value : 0,
)

const netSalesChartPoints = computed<ChartPoint[]>(() =>
  makeChartPoints(dailySeries.value, (item) => item.netSalesAmount),
)

const grossProfitChartPoints = computed<ChartPoint[]>(() =>
  makeChartPoints(dailySeries.value, (item) => item.grossProfitAmount),
)

const netSalesChartPath = computed(() => makeLinePath(netSalesChartPoints.value))
const grossProfitChartPath = computed(() => makeLinePath(grossProfitChartPoints.value))

const chartAreaPath = computed(() => {
  const points = netSalesChartPoints.value
  if (points.length === 0) {
    return ''
  }
  const baseline = chartValueY(0)
  return `${makeLinePath(points)} L ${points[points.length - 1].x} ${baseline} L ${points[0].x} ${baseline} Z`
})

const chartTicks = computed<ChartTick[]>(() =>
  {
    const tickCount = chartScaleMin.value < 0 ? 5 : 4
    return Array.from({ length: tickCount }, (_, index) => {
      const ratio = index / (tickCount - 1)
      const value = chartScaleMax.value - (chartScaleMax.value - chartScaleMin.value) * ratio
      return {
        value,
        y: chartValueY(value),
      }
    })
  },
)

const chartLabels = computed<ChartLabel[]>(() => {
  const series = dailySeries.value
  if (series.length === 0) {
    return []
  }
  if (series.length === 1) {
    return [{ label: series[0].label.slice(5), x: CHART_PADDING.left }]
  }
  const labelIndexes = new Set([0, Math.floor((series.length - 1) / 2), series.length - 1])
  return [...labelIndexes].sort((left, right) => left - right).map((index) => ({
    label: series[index].label.slice(5),
    x: CHART_PADDING.left + (chartInnerWidth * index) / (series.length - 1),
  }))
})

const rangeOutboundAmount = computed(() =>
  dailySeries.value.reduce((sum, item) => sum + item.outboundAmount, 0),
)

const rangeReturnAmount = computed(() =>
  dailySeries.value.reduce((sum, item) => sum + item.returnAmount, 0),
)

const rangeOutboundQuantity = computed(() =>
  data.orders
    .filter((order) => dashboardDateMatched(order.deliveryDate))
    .reduce((sum, order) => sum + order.quantity, 0),
)
const rangeReturnQuantity = computed(() =>
  data.returns
    .filter((record) => dashboardDateMatched(record.returnTime))
    .reduce((sum, record) => sum + record.quantity, 0),
)
const rangeNetAmount = computed(() => rangeOutboundAmount.value - rangeReturnAmount.value)
const rangeGrossProfitAmount = computed(() =>
  dailySeries.value.reduce((sum, item) => sum + item.grossProfitAmount, 0),
)
const rangeGrossProfitRate = computed(() => percentOf(rangeGrossProfitAmount.value, rangeNetAmount.value))
const rangeReturnImpactRate = computed(() => percentOf(rangeReturnAmount.value, rangeOutboundAmount.value))
const rangeInvoiceAmount = computed(() =>
  data.invoices
    .filter((invoice) => dashboardDateMatched(invoice.invoiceDate))
    .reduce((sum, invoice) => sum + invoice.totalAmount, 0),
)
const rangePaidInvoiceAmount = computed(() =>
  data.invoices
    .filter((invoice) => invoice.isPaid && dashboardDateMatched(invoice.invoiceDate))
    .reduce((sum, invoice) => sum + invoice.totalAmount, 0),
)
const rangeUnpaidInvoiceAmount = computed(() => rangeInvoiceAmount.value - rangePaidInvoiceAmount.value)
const rangePaidRate = computed(() => percentOf(rangePaidInvoiceAmount.value, rangeInvoiceAmount.value))
const chartHasData = computed(() => dailySeries.value.some((item) => item.outboundAmount > 0 || item.returnAmount > 0))
const pieTotalAmount = computed(() => Math.max(0, rangeOutboundAmount.value) + Math.max(0, rangeReturnAmount.value))

const pieSlices = computed<PieSlice[]>(() => {
  const values = [
    { label: '出库金额', value: Math.max(0, rangeOutboundAmount.value), className: 'outbound' },
    { label: '退货金额', value: Math.max(0, rangeReturnAmount.value), className: 'returns' },
  ]
  const total = pieTotalAmount.value
  let offset = 0
  return values.map((item) => {
    const percent = total > 0 ? item.value / total : 0
    const length = percent * PIE_CIRCUMFERENCE
    const slice = {
      ...item,
      percent,
      dashArray: `${length} ${PIE_CIRCUMFERENCE - length}`,
      dashOffset: -offset,
    }
    offset += length
    return slice
  })
})

function loadData(): AppData {
  const saved = localStorage.getItem(STORAGE_KEY)
  if (!saved) {
    return defaultData()
  }
  try {
    const loaded = { ...defaultData(), ...JSON.parse(saved) } as AppData
    loaded.products = loaded.products.map((product) => ({
      ...product,
      unit: product.unit || '件',
    }))
    loaded.customers = Array.isArray(loaded.customers) ? loaded.customers : []
    loaded.orders = Array.isArray(loaded.orders)
      ? loaded.orders.map((order) => normalizeLoadedOrder(order, loaded.customers))
      : []
    loaded.invoices = Array.isArray(loaded.invoices) ? loaded.invoices.map(normalizeLoadedInvoice) : []
    loaded.returns = Array.isArray(loaded.returns) ? loaded.returns : []
    syncInvoiceBackfills(loaded.orders, loaded.invoices, false)
    return loaded
  } catch {
    return defaultData()
  }
}

function normalizeLoadedOrder(order: Partial<Order>, customers: Customer[]): Order {
  const orderTime = normalizeDate(order.orderTime ?? '') || todayString()
  const deliveryDate = normalizeDate(order.deliveryDate ?? '') || orderTime
  const itemNo = order.itemNo?.trim() || 'ITEM'
  const invoiceTotal = Number(order.invoiceTotal ?? 0)
  return {
    id: order.id || createId(),
    orderNo: order.orderNo || generateOrderNo(itemNo),
    productId: order.productId ?? null,
    customerId: order.customerId ?? findCustomerIdByText(customers, order.customerUnit ?? '', order.customerName ?? ''),
    month: order.month || monthFromDate(orderTime),
    productName: order.productName ?? '',
    itemNo,
    orderTime,
    deliveryDate,
    customerUnit: order.customerUnit ?? '',
    customerName: order.customerName ?? '',
    brand: order.brand ?? '',
    unit: order.unit || '件',
    catalogPrice: Number(order.catalogPrice ?? 0),
    quantity: Number(order.quantity ?? 0),
    invoiceTotal,
    invoiceStatus: order.invoiceStatus || (order.invoiceNo ? '全部开票' : '未开票'),
    isShipped: order.isShipped ?? true,
    cashback: Number(order.cashback ?? 0),
    costDiscount: Number(order.costDiscount ?? 0),
    costUnitPrice: Number(order.costUnitPrice ?? 0),
    costTotal: Number(order.costTotal ?? 0),
    saleDiscount: Number(order.saleDiscount ?? 0),
    invoiceUnitPrice: Number(order.invoiceUnitPrice ?? 0),
    grossProfit: Number(order.grossProfit ?? 0),
    remark: order.remark ?? '',
    invoiceNo: order.invoiceNo ?? '',
    isPaid: order.isPaid ?? Boolean(order.paidTime),
    paidTime: normalizeDate(order.paidTime ?? '') || '',
    returnedQuantity: Number(order.returnedQuantity ?? 0),
    createdAt: order.createdAt || nowString(),
  }
}

function normalizeLoadedInvoice(invoice: Partial<Invoice>): Invoice {
  const lines = Array.isArray(invoice.lines) ? invoice.lines : []
  const normalizedLines: InvoiceLine[] = lines.map((line) => ({
    id: line.id || createId(),
    orderId: line.orderId,
    orderNo: line.orderNo || '',
    productName: line.productName || '',
    itemNo: line.itemNo || '',
    customerUnit: line.customerUnit || '',
    customerName: line.customerName || '',
    orderAmount: Number(line.orderAmount ?? 0),
    invoiceAmount: Number(line.invoiceAmount ?? 0),
  }))
  return {
    id: invoice.id || createId(),
    invoiceNo: invoice.invoiceNo || generateInvoiceNo(),
    invoiceDate: normalizeDate(invoice.invoiceDate ?? '') || todayString(),
    isPaid: invoice.isPaid ?? false,
    paidTime: normalizeDate(invoice.paidTime ?? '') || '',
    totalAmount: Number(invoice.totalAmount ?? normalizedLines.reduce((sum, line) => sum + line.invoiceAmount, 0)),
    remark: invoice.remark || '',
    lines: normalizedLines,
    createdAt: invoice.createdAt || nowString(),
  }
}

function blankProductForm(): ProductForm {
  return {
    name: '',
    purchasePrice: '0',
    outboundPrice: '0',
    specification: '',
    brand: '',
    unit: '件',
    description: '',
  }
}

function blankCustomerForm(): CustomerForm {
  return {
    unit: '',
    contactName: '',
    phone: '',
    address: '',
    remark: '',
  }
}

function blankBatchOrderLine(): BatchOrderLine {
  return {
    id: createId(),
    productId: null,
    productName: '',
    itemNo: '',
    brand: '',
    unit: '件',
    quantity: '1',
    invoiceUnitPrice: '0',
    costUnitPrice: '0',
    invoiceTotal: '0',
    costTotal: '0',
    cashback: '0',
    grossProfit: '0',
    remark: '',
  }
}

function blankOrderForm(): OrderForm {
  return {
    orderNo: '',
    productId: null,
    customerId: null,
    month: monthFromDate(todayString()),
    productName: '',
    itemNo: '',
    orderTime: todayString(),
    deliveryDate: todayString(),
    customerUnit: '',
    customerName: '',
    brand: '',
    unit: '件',
    catalogPrice: '0',
    quantity: '1',
    invoiceTotal: '0',
    invoiceStatus: '未开票',
    isShipped: true,
    cashback: '0',
    costDiscount: '0',
    costUnitPrice: '0',
    costTotal: '0',
    saleDiscount: '0',
    invoiceUnitPrice: '0',
    grossProfit: '0',
    remark: '',
    invoiceNo: '',
    isPaid: false,
    paidTime: '',
  }
}

function blankInvoiceForm(): InvoiceForm {
  return {
    invoiceNo: generateInvoiceNo(),
    invoiceDate: todayString(),
    isPaid: false,
    paidTime: '',
    remark: '',
    lines: [],
  }
}

function blankReturnForm(): ReturnForm {
  return {
    sourceOrderId: null,
    productId: null,
    productName: '',
    itemNo: '',
    returnTime: todayString(),
    customerUnit: '',
    customerName: '',
    brand: '',
    unit: '件',
    catalogPrice: '0',
    quantity: '1',
    invoiceTotal: '0',
    cashback: '0',
    costDiscount: '0',
    costUnitPrice: '0',
    costTotal: '0',
    saleDiscount: '0',
    invoiceUnitPrice: '0',
    grossProfit: '0',
    remark: '',
    paidTime: todayString(),
  }
}

function addDays(dateText: string, offset: number): string {
  const date = new Date(`${dateText}T00:00:00`)
  date.setDate(date.getDate() + offset)
  return formatLocalDate(date)
}

function matchesQuery(query: string, fields: string[]): boolean {
  const text = query.trim().toLowerCase()
  return !text || fields.some((field) => field.toLowerCase().includes(text))
}

function paginate<T>(rows: T[], page: number, pageSize: number): T[] {
  const start = (page - 1) * pageSize
  return rows.slice(start, start + pageSize)
}

function dateInRange(dateText: string, range: [string, string] | []): boolean {
  if (range.length !== 2) {
    return true
  }
  const value = dateValue(normalizeDate(dateText))
  const start = dateValue(range[0])
  const end = dateValue(range[1])
  return value >= start && value <= end
}

function dashboardDateMatched(dateText: string): boolean {
  const value = dateValue(normalizeDate(dateText))
  const start = dateValue(dashboardFilter.startDate)
  const end = dateValue(dashboardFilter.endDate)
  return Boolean(value && start && end && value >= start && value <= end)
}

function monthFromDate(dateText: string): string {
  const normalized = normalizeDate(dateText) || todayString()
  return normalized.slice(0, 7)
}

function normalizeImportMonth(value: string | undefined, orderTime: string): string {
  const text = String(value ?? '').trim()
  if (!text) {
    return monthFromDate(orderTime)
  }
  const numericMonth = Number(text)
  if (Number.isInteger(numericMonth) && numericMonth >= 1 && numericMonth <= 12) {
    return String(numericMonth)
  }
  return text
}

function yesNo(value: boolean): string {
  return value ? '是' : '否'
}

function parseYesNo(value: string | undefined, defaultValue = false): boolean {
  const text = String(value ?? '').trim()
  if (!text) {
    return defaultValue
  }
  return ['是', '已发货', '已回款', 'true', '1', 'yes', 'y'].includes(text.toLowerCase())
}

function normalizePrintProfileKey(value: string | null): PrintProfileKey {
  return value === 'a4Portrait' || value === 'a4Landscape' ? 'a4Portrait' : 'dotMatrix'
}

function loadPrintContentSettings(): PrintContentSettings {
  try {
    const saved = localStorage.getItem(PRINT_CONTENT_SETTINGS_STORAGE_KEY)
    if (!saved) {
      return { ...DEFAULT_PRINT_CONTENT_SETTINGS }
    }
    const parsed = JSON.parse(saved) as Partial<PrintContentSettings>
    return normalizePrintContentSettings(parsed)
  } catch {
    return { ...DEFAULT_PRINT_CONTENT_SETTINGS }
  }
}

function normalizePrintContentSettings(value: Partial<PrintContentSettings>): PrintContentSettings {
  return {
    companyName: String(value.companyName ?? DEFAULT_PRINT_CONTENT_SETTINGS.companyName),
    documentTitle: String(value.documentTitle ?? DEFAULT_PRINT_CONTENT_SETTINGS.documentTitle),
    orderNoPrefix: String(value.orderNoPrefix ?? DEFAULT_PRINT_CONTENT_SETTINGS.orderNoPrefix),
    warehouseName: String(value.warehouseName ?? DEFAULT_PRINT_CONTENT_SETTINGS.warehouseName),
    receiverSignatureLabel: String(value.receiverSignatureLabel ?? DEFAULT_PRINT_CONTENT_SETTINGS.receiverSignatureLabel),
    receiverSignatureHint: String(value.receiverSignatureHint ?? DEFAULT_PRINT_CONTENT_SETTINGS.receiverSignatureHint),
  }
}

function resetPrintContentSettings(): void {
  Object.assign(printContentSettings, DEFAULT_PRINT_CONTENT_SETTINGS)
}

function generateOrderItemNo(product?: Product, _sequenceOffset = 0): string {
  return product?.specification?.trim() || product?.name?.trim() || 'ITEM'
}

function random4(): string {
  return String(Math.floor(Math.random() * 10000)).padStart(4, '0')
}

function generateOrderNo(itemNo: string): string {
  const prefix = itemNo.trim() || 'ITEM'
  return `${prefix}-${nowCompactString()}${random4()}`
}

function generateInvoiceNo(): string {
  return `FP-${nowCompactString()}${random4()}`
}

function invoiceStatusForAmount(invoiceAmount: number, orderAmount: number): string {
  if (invoiceAmount <= 0) {
    return '未开票'
  }
  return invoiceAmount + Number.EPSILON >= orderAmount ? '全部开票' : '部分开票'
}

function syncInvoiceBackfills(orders: Order[] = data.orders, invoices: Invoice[] = data.invoices, clearUnrelated = true): void {
  orders.forEach((order) => {
    const related = invoices.filter((invoice) => invoice.lines.some((line) => line.orderId === order.id))
    const invoiceAmount = related.reduce(
      (sum, invoice) => sum + invoice.lines.filter((line) => line.orderId === order.id).reduce((lineSum, line) => lineSum + Number(line.invoiceAmount || 0), 0),
      0,
    )
    if (!related.length && !clearUnrelated) {
      return
    }
    order.invoiceStatus = invoiceStatusForAmount(invoiceAmount, order.invoiceTotal)
    order.invoiceNo = related.map((invoice) => invoice.invoiceNo).filter(Boolean).join('、')
    if (related.length > 0) {
      order.isPaid = related.every((invoice) => invoice.isPaid)
      order.paidTime = related.every((invoice) => invoice.isPaid) ? related.map((invoice) => invoice.paidTime).filter(Boolean).sort().at(-1) ?? '' : ''
    } else {
      order.isPaid = false
      order.paidTime = ''
    }
  })
}

function summarizeAmounts(rows: AmountSummaryRow[]): AmountSummary {
  const summary = rows.reduce<AmountSummary>(
    (result, row) => {
      result.quantity += row.quantity
      result.invoiceTotal += row.invoiceTotal
      result.cashback += row.cashback
      result.costTotal += row.costTotal
      result.grossProfit += row.grossProfit
      return result
    },
    {
      quantity: 0,
      invoiceTotal: 0,
      cashback: 0,
      costTotal: 0,
      grossProfit: 0,
      averageInvoiceUnitPrice: 0,
      averageCostUnitPrice: 0,
    },
  )

  if (summary.quantity > 0) {
    summary.averageInvoiceUnitPrice = summary.invoiceTotal / summary.quantity
    summary.averageCostUnitPrice = summary.costTotal / summary.quantity
  }
  return summary
}

function updateViewportHeight(): void {
  viewportHeight.value = window.innerHeight
  scheduleTableHeightUpdate()
}

function scheduleTableHeightUpdate(): void {
  cancelAnimationFrame(tableHeightFrame)
  tableHeightFrame = requestAnimationFrame(() => {
    void nextTick(updateTableHeight)
  })
}

function updateTableHeight(): void {
  const panel = document.querySelector('.table-panel') as HTMLElement | null
  const table = panel?.querySelector('.el-table') as HTMLElement | null
  const pagination = panel?.querySelector('.pagination-bar') as HTMLElement | null
  const amountTotal = panel?.querySelector('.amount-total-bar') as HTMLElement | null
  if (!table) {
    return
  }

  const top = table.getBoundingClientRect().top
  const paginationHeight = pagination?.offsetHeight ?? 48
  const amountTotalHeight = amountTotal?.offsetHeight ?? 0
  tableHeight.value = Math.max(300, Math.floor(window.innerHeight - top - paginationHeight - amountTotalHeight - 28))
}

function toggleTheme(): void {
  themeMode.value = isDarkMode.value ? 'light' : 'dark'
}

function navCount(key: string): string {
  if (key === 'products') {
    return String(data.products.length)
  }
  if (key === 'customers') {
    return String(data.customers.length)
  }
  if (key === 'orders') {
    return String(data.orders.length)
  }
  if (key === 'invoices') {
    return String(data.invoices.length)
  }
  if (key === 'returns') {
    return String(data.returns.length)
  }
  return String(data.products.length + data.customers.length + data.orders.length + data.invoices.length + data.returns.length)
}

function identity(value: string): string {
  return value.trim().toLowerCase()
}

function identityNumber(value: number): string {
  return Number.isFinite(value) ? value.toFixed(6).replace(/\.?0+$/, '') : '0'
}

function orderImportKey(order: Pick<Order,
  | 'month'
  | 'productName'
  | 'itemNo'
  | 'orderTime'
  | 'customerUnit'
  | 'customerName'
  | 'brand'
  | 'unit'
  | 'catalogPrice'
  | 'quantity'
  | 'cashback'
  | 'costDiscount'
  | 'costUnitPrice'
  | 'saleDiscount'
  | 'invoiceUnitPrice'
  | 'remark'
  | 'invoiceNo'
  | 'isShipped'
  | 'returnedQuantity'
>): string {
  return [
    identity(order.month),
    identity(order.productName),
    identity(order.itemNo),
    normalizeDate(order.orderTime) || identity(order.orderTime),
    identity(order.customerUnit),
    identity(order.customerName),
    identity(order.brand),
    identity(order.unit),
    identityNumber(order.catalogPrice),
    identityNumber(order.quantity),
    identityNumber(order.cashback),
    identityNumber(order.costDiscount),
    identityNumber(order.costUnitPrice),
    identityNumber(order.saleDiscount),
    identityNumber(order.invoiceUnitPrice),
    identity(order.remark),
    identity(order.invoiceNo),
    order.isShipped ? '1' : '0',
    identityNumber(order.returnedQuantity),
  ].join('|')
}

function findCustomerIdByText(customers: Customer[], unit: string, contactName: string): string | null {
  const customer = customers.find(
    (item) => identity(item.unit) === identity(unit) && identity(item.contactName) === identity(contactName),
  )
  return customer?.id ?? null
}

function ensureCustomerForOrderImport(unit: string, contactName: string): Customer | null {
  const normalizedUnit = unit.trim()
  const normalizedContactName = contactName.trim()
  if (!normalizedUnit && !normalizedContactName) {
    return null
  }

  const existing = data.customers.find(
    (item) => identity(item.unit) === identity(normalizedUnit) && identity(item.contactName) === identity(normalizedContactName),
  )
  if (existing) {
    return existing
  }

  const customer: Customer = {
    id: createId(),
    unit: normalizedUnit,
    contactName: normalizedContactName,
    phone: '',
    address: '',
    remark: '出库单导入自动创建',
    createdAt: nowString(),
  }
  data.customers.push(customer)
  return customer
}

function findProductForOrderImport(productName: string, brand: string, itemNo: string): Product | null {
  const normalizedItemNo = identity(itemNo)
  const exact = data.products.find(
    (item) =>
      identity(item.name) === identity(productName)
      && identity(item.brand) === identity(brand)
      && identity(item.specification) === normalizedItemNo,
  )
  if (exact) {
    return exact
  }
  if (normalizedItemNo) {
    return null
  }

  return data.products.find(
    (item) => identity(item.name) === identity(productName) && identity(item.brand) === identity(brand),
  ) ?? null
}

function ensureProductForOrderImport(
  productName: string,
  brand: string,
  itemNo: string,
  unit: string,
  purchasePrice: number,
  outboundPrice: number,
): Product {
  const existing = findProductForOrderImport(productName, brand, itemNo)
  if (existing) {
    return existing
  }

  const product: Product = {
    id: createId(),
    name: productName,
    purchasePrice,
    outboundPrice,
    specification: itemNo,
    brand,
    unit: unit || '件',
    description: '出库单导入自动创建',
    createdAt: nowString(),
  }
  data.products.push(product)
  return product
}

function customerOrders(customer: Customer): Order[] {
  return data.orders.filter(
    (order) =>
      order.customerId === customer.id
      || (!order.customerId && identity(order.customerUnit) === identity(customer.unit) && identity(order.customerName) === identity(customer.contactName)),
  )
}

function customerOrderCount(customer: Customer): number {
  return customerOrders(customer).length
}

function customerOrderQuantity(customer: Customer): number {
  return customerOrders(customer).reduce((sum, order) => sum + order.quantity, 0)
}

function customerReturns(customer: Customer): ReturnRecord[] {
  return data.returns.filter(
    (record) => identity(record.customerUnit) === identity(customer.unit) && identity(record.customerName) === identity(customer.contactName),
  )
}

function customerReturnQuantity(customer: Customer): number {
  return customerReturns(customer).reduce((sum, record) => sum + record.quantity, 0)
}

function openOrdersForCustomer(customer: Customer): void {
  activePage.value = 'orders'
  orderQuery.value = customer.unit
  orderPage.value = 1
  statusMessage.value = `已筛选客户下单：${customer.unit}`
  scheduleTableHeightUpdate()
}

function openOrdersForInvoice(invoice: Invoice): void {
  activePage.value = 'orders'
  orderQuery.value = invoice.invoiceNo
  orderPage.value = 1
  statusMessage.value = `已筛选发票关联出库单：${invoice.invoiceNo}`
  scheduleTableHeightUpdate()
}

function openReturnsForCustomer(customer: Customer): void {
  activePage.value = 'returns'
  returnQuery.value = customer.unit
  returnPage.value = 1
  statusMessage.value = `已筛选客户退货：${customer.unit}`
  scheduleTableHeightUpdate()
}

function markSaved(message: string): void {
  data.savedAt = nowString()
  statusMessage.value = message
  ElMessage.success(message)
}

function parseNonNegative(label: string, value: string): number {
  const parsed = parseAmount(value)
  if (parsed < 0) {
    throw new Error(`${label}不能小于 0`)
  }
  return parsed
}

function parseImportCashback(value: string | undefined): number {
  const text = String(value ?? '').trim()
  if (!text) {
    return 0
  }
  if (['是', '否', '已返现', '未返现', 'true', 'false', 'yes', 'no', 'y', 'n'].includes(text.toLowerCase())) {
    return 0
  }
  return parseNonNegative('返现', text)
}

function parsePositive(label: string, value: string): number {
  const parsed = parseNonNegative(label, value)
  if (parsed <= 0) {
    throw new Error(`${label}必须大于 0`)
  }
  return parsed
}

function parseAmountForAuto(value: string): number | null {
  try {
    const parsed = parseAmount(value)
    return parsed >= 0 ? parsed : null
  } catch {
    return null
  }
}

function syncAmountFields(
  quantityText: string,
  invoiceUnitPriceText: string,
  costUnitPriceText: string,
  cashbackText: string,
  target: Pick<OrderForm, 'invoiceTotal' | 'costTotal' | 'grossProfit'>,
): boolean {
  const quantity = parseAmountForAuto(quantityText)
  const invoiceUnitPrice = parseAmountForAuto(invoiceUnitPriceText)
  const costUnitPrice = parseAmountForAuto(costUnitPriceText)
  if (quantity === null || invoiceUnitPrice === null || costUnitPrice === null) {
    return false
  }

  const invoiceTotal = quantity * invoiceUnitPrice
  const costTotal = quantity * costUnitPrice
  target.invoiceTotal = formatMoney(invoiceTotal)
  target.costTotal = formatMoney(costTotal)

  const cashback = parseAmountForAuto(cashbackText) ?? 0
  target.grossProfit = formatMoney(invoiceTotal - costTotal - cashback)
  return true
}

function syncOrderAmounts(): boolean {
  return syncAmountFields(
    orderForm.quantity,
    orderForm.invoiceUnitPrice,
    orderForm.costUnitPrice,
    orderForm.cashback,
    orderForm,
  )
}

function syncReturnAmounts(): boolean {
  return syncAmountFields(
    returnForm.quantity,
    returnForm.invoiceUnitPrice,
    returnForm.costUnitPrice,
    returnForm.cashback,
    returnForm,
  )
}

function safeAction(action: () => void): void {
  try {
    action()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    statusMessage.value = message
    ElMessage.error(message)
  }
}

function saveProduct(): void {
  safeAction(() => {
    const name = productForm.name.trim()
    if (!name) {
      throw new Error('产品名称不能为空')
    }
    const duplicate = data.products.some(
      (product) =>
        product.id !== editingProductId.value
        && identity(product.name) === identity(name)
        && identity(product.brand) === identity(productForm.brand)
        && identity(product.specification) === identity(productForm.specification),
    )
    if (duplicate) {
      throw new Error('相同名称、品牌、规格/型号的产品已存在')
    }

    const next: Product = {
      id: editingProductId.value ?? createId(),
      name,
      purchasePrice: parseNonNegative('进货金额', productForm.purchasePrice),
      outboundPrice: parseNonNegative('出库金额', productForm.outboundPrice),
      specification: productForm.specification.trim(),
      brand: productForm.brand.trim(),
      unit: productForm.unit.trim() || '件',
      description: productForm.description.trim(),
      createdAt: data.products.find((product) => product.id === editingProductId.value)?.createdAt ?? nowString(),
    }

    if (editingProductId.value) {
      const index = data.products.findIndex((product) => product.id === editingProductId.value)
      data.products.splice(index, 1, next)
      editingProductId.value = null
      Object.assign(productForm, blankProductForm())
      productDialogVisible.value = false
      markSaved('产品已更新')
      return
    }

    data.products.push(next)
    Object.assign(productForm, blankProductForm())
    productDialogVisible.value = false
    markSaved('产品已新增')
  })
}

function openProductCreate(): void {
  editingProductId.value = null
  Object.assign(productForm, blankProductForm())
  productDialogVisible.value = true
}

function editProduct(product: Product): void {
  Object.assign(productForm, {
    name: product.name,
    purchasePrice: formatMoney(product.purchasePrice),
    outboundPrice: formatMoney(product.outboundPrice),
    specification: product.specification,
    brand: product.brand,
    unit: product.unit || '件',
    description: product.description,
  })
  editingProductId.value = product.id
  productDialogVisible.value = true
  activePage.value = 'products'
  statusMessage.value = `正在编辑产品：${product.name}`
}

async function deleteProduct(product: Product): Promise<void> {
  if (data.orders.some((order) => order.productId === product.id) || data.returns.some((record) => record.productId === product.id)) {
    ElMessage.warning('该产品已被订单或退货引用')
    return
  }
  if (!(await confirmDanger(`确认删除产品“${product.name}”？`, '删除产品'))) {
    return
  }
  data.products = data.products.filter((item) => item.id !== product.id)
  markSaved('产品已删除')
}

function saveCustomer(): void {
  safeAction(() => {
    const unit = customerForm.unit.trim()
    const contactName = customerForm.contactName.trim()
    if (!unit) {
      throw new Error('客户单位不能为空')
    }

    const duplicate = data.customers.some(
      (customer) =>
        customer.id !== editingCustomerId.value
        && identity(customer.unit) === identity(unit)
        && identity(customer.contactName) === identity(contactName),
    )
    if (duplicate) {
      throw new Error('相同单位和联系人客户已存在')
    }

    const next: Customer = {
      id: editingCustomerId.value ?? createId(),
      unit,
      contactName,
      phone: customerForm.phone.trim(),
      address: customerForm.address.trim(),
      remark: customerForm.remark.trim(),
      createdAt: data.customers.find((customer) => customer.id === editingCustomerId.value)?.createdAt ?? nowString(),
    }

    if (editingCustomerId.value) {
      const index = data.customers.findIndex((customer) => customer.id === editingCustomerId.value)
      data.customers.splice(index, 1, next)
      data.orders.forEach((order) => {
        if (order.customerId === next.id) {
          order.customerUnit = next.unit
          order.customerName = next.contactName
        }
      })
      resetCustomerForm()
      markSaved('客户已更新')
      return
    }

    data.customers.push(next)
    resetCustomerForm()
    markSaved('客户已新增')
  })
}

function openCustomerCreate(): void {
  editingCustomerId.value = null
  Object.assign(customerForm, blankCustomerForm())
  customerDialogVisible.value = true
}

function editCustomer(customer: Customer): void {
  Object.assign(customerForm, {
    unit: customer.unit,
    contactName: customer.contactName,
    phone: customer.phone,
    address: customer.address,
    remark: customer.remark,
  })
  editingCustomerId.value = customer.id
  customerDialogVisible.value = true
  activePage.value = 'customers'
}

async function deleteCustomer(customer: Customer): Promise<void> {
  if (data.orders.some((order) => order.customerId === customer.id)) {
    ElMessage.warning('该客户已被订单引用')
    return
  }
  if (!(await confirmDanger(`确认删除客户“${customer.unit}”？`, '删除客户'))) {
    return
  }
  data.customers = data.customers.filter((item) => item.id !== customer.id)
  markSaved('客户已删除')
}

function openBatchOrderForCustomer(customer: Customer): void {
  batchOrderCustomerId.value = customer.id
  batchOrderTime.value = todayString()
  batchDeliveryDate.value = todayString()
  batchPaidTime.value = todayString()
  batchOrderRows.value = [blankBatchOrderLine()]
  batchOrderDialogVisible.value = true
  activePage.value = 'customers'
  statusMessage.value = `正在为客户下单：${customer.unit}`
}

function resetBatchOrderForm(): void {
  batchOrderDialogVisible.value = false
  batchOrderCustomerId.value = null
  batchOrderTime.value = todayString()
  batchDeliveryDate.value = todayString()
  batchPaidTime.value = todayString()
  batchOrderRows.value = []
}

function addBatchOrderRow(): void {
  batchOrderRows.value.push(blankBatchOrderLine())
  scheduleTableHeightUpdate()
}

function removeBatchOrderRow(rowId: string): void {
  if (batchOrderRows.value.length <= 1) {
    ElMessage.warning('至少保留一条产品明细')
    return
  }
  batchOrderRows.value = batchOrderRows.value.filter((row) => row.id !== rowId)
}

function applyProductToBatchRow(row: BatchOrderLine): void {
  const product = data.products.find((item) => item.id === row.productId)
  if (!product) {
    row.productName = ''
    row.itemNo = ''
    row.brand = ''
    row.unit = '件'
    return
  }

  const rowIndex = Math.max(0, batchOrderRows.value.findIndex((item) => item.id === row.id))
  row.productName = product.name
  row.itemNo = generateOrderItemNo(product, rowIndex)
  row.brand = product.brand
  row.unit = product.unit || '件'
  row.invoiceUnitPrice = formatMoney(product.outboundPrice)
  row.costUnitPrice = formatMoney(product.purchasePrice)
  syncBatchOrderRowAmounts(row)
}

function syncBatchOrderRowAmounts(row: BatchOrderLine): boolean {
  const quantity = parseAmountForAuto(row.quantity)
  const invoiceUnitPrice = parseAmountForAuto(row.invoiceUnitPrice)
  const costUnitPrice = parseAmountForAuto(row.costUnitPrice)
  if (quantity === null || invoiceUnitPrice === null || costUnitPrice === null) {
    return false
  }

  const invoiceTotal = quantity * invoiceUnitPrice
  const costTotal = quantity * costUnitPrice
  const cashback = parseAmountForAuto(row.cashback) ?? 0
  row.invoiceTotal = formatMoney(invoiceTotal)
  row.costTotal = formatMoney(costTotal)
  row.grossProfit = formatMoney(invoiceTotal - costTotal - cashback)
  return true
}

function saveBatchOrders(): void {
  safeAction(() => {
    const customer = batchOrderCustomer.value
    if (!customer) {
      throw new Error('请先选择客户')
    }

    const orderTime = normalizeDate(batchOrderTime.value)
    const deliveryDate = normalizeDate(batchDeliveryDate.value)
    const paidTime = normalizeDate(batchPaidTime.value) || todayString()
    if (!orderTime || !deliveryDate) {
      throw new Error('订购时间和出库日期不能为空')
    }
    if (dateValue(deliveryDate) < dateValue(orderTime)) {
      throw new Error('出库日期不能早于订购日期')
    }

    const validRows = batchOrderRows.value.filter((row) => row.productId || row.productName.trim())
    if (!validRows.length) {
      throw new Error('请至少添加一个产品')
    }

    validRows.forEach((row, index) => {
      syncBatchOrderRowAmounts(row)
      const product = data.products.find((item) => item.id === row.productId)
      const productName = row.productName.trim() || product?.name || ''
      const itemNo = row.itemNo.trim() || generateOrderItemNo(product, index)
      if (!productName) {
        throw new Error(`第 ${index + 1} 行产品不能为空`)
      }

      const quantity = parsePositive(`第 ${index + 1} 行数量`, row.quantity)
      const invoiceUnitPrice = parseNonNegative(`第 ${index + 1} 行销售单价`, row.invoiceUnitPrice)
      const costUnitPrice = parseNonNegative(`第 ${index + 1} 行成本价`, row.costUnitPrice)
      const invoiceTotal = parseNonNegative(`第 ${index + 1} 行销售总价`, row.invoiceTotal)
      const costTotal = parseNonNegative(`第 ${index + 1} 行成本总价`, row.costTotal)
      const cashback = parseNonNegative(`第 ${index + 1} 行返现`, row.cashback)

      data.orders.push({
        id: createId(),
        orderNo: generateOrderNo(itemNo),
        productId: row.productId,
        customerId: customer.id,
        month: monthFromDate(orderTime),
        productName,
        itemNo,
        orderTime,
        deliveryDate,
        customerUnit: customer.unit,
        customerName: customer.contactName,
        brand: row.brand.trim() || product?.brand || '',
        unit: row.unit.trim() || product?.unit || '件',
        catalogPrice: invoiceUnitPrice,
        quantity,
        invoiceTotal,
        invoiceStatus: '未开票',
        isShipped: true,
        cashback,
        costDiscount: 0,
        costUnitPrice,
        costTotal,
        saleDiscount: 0,
        invoiceUnitPrice,
        grossProfit: invoiceTotal - costTotal - cashback,
        remark: row.remark.trim(),
        invoiceNo: '',
        isPaid: false,
        paidTime: '',
        returnedQuantity: 0,
        createdAt: nowString(),
      })
    })

    const count = validRows.length
    resetBatchOrderForm()
    activePage.value = 'orders'
    orderQuery.value = customer.unit
    orderPage.value = 1
    markSaved(`已为 ${customer.unit} 新增 ${count} 条订单`)
    scheduleTableHeightUpdate()
  })
}

function applyProductToOrder(productId: string): void {
  const product = data.products.find((item) => item.id === productId)
  if (!product) {
    return
  }
  orderForm.productId = product.id
  orderForm.productName = product.name
  orderForm.itemNo = generateOrderItemNo(product)
  orderForm.brand = product.brand
  orderForm.unit = product.unit || '件'
  orderForm.catalogPrice = formatMoney(product.outboundPrice)
  orderForm.invoiceUnitPrice = formatMoney(product.outboundPrice)
  orderForm.costUnitPrice = formatMoney(product.purchasePrice)
  recalculateOrderAmounts(false)
}

function applyCustomerToOrder(customerId: string): void {
  const customer = data.customers.find((item) => item.id === customerId)
  if (!customer) {
    orderForm.customerId = null
    return
  }
  orderForm.customerId = customer.id
  orderForm.customerUnit = customer.unit
  orderForm.customerName = customer.contactName
}

function prepareOrder(product: Product): void {
  Object.assign(orderForm, blankOrderForm())
  applyProductToOrder(product.id)
  editingOrderId.value = null
  orderDialogVisible.value = true
  activePage.value = 'orders'
}

function openOrdersForProduct(product: Product): void {
  activePage.value = 'orders'
  orderQuery.value = product.name
  orderPage.value = 1
  statusMessage.value = `已筛选产品订单：${product.name}`
  scheduleTableHeightUpdate()
}

function recalculateOrderAmounts(showMessage = true): void {
  safeAction(() => {
    const quantity = parsePositive('数量', orderForm.quantity)
    const invoiceUnitPrice = parseNonNegative('销售单价', orderForm.invoiceUnitPrice)
    const costUnitPrice = parseNonNegative('成本单价', orderForm.costUnitPrice)
    const cashback = parseNonNegative('返现', orderForm.cashback)
    const invoiceTotal = quantity * invoiceUnitPrice
    const costTotal = quantity * costUnitPrice
    orderForm.invoiceTotal = formatMoney(invoiceTotal)
    orderForm.costTotal = formatMoney(costTotal)
    orderForm.grossProfit = formatMoney(invoiceTotal - costTotal - cashback)
    if (showMessage) {
      statusMessage.value = '订单金额已重算'
    }
  })
}

function buildOrder(id: string, returnedQuantity: number, createdAt: string): Order {
  syncOrderAmounts()
  const orderTime = normalizeDate(orderForm.orderTime)
  const deliveryDate = normalizeDate(orderForm.deliveryDate)
  if (!orderForm.productName.trim()) {
    throw new Error('产品名称不能为空')
  }
  if (!orderTime || !deliveryDate) {
    throw new Error('订购时间和出库日期不能为空')
  }
  if (dateValue(deliveryDate) < dateValue(orderTime)) {
    throw new Error('出库日期不能早于订购日期')
  }

  const quantity = parsePositive('数量', orderForm.quantity)
  if (quantity < returnedQuantity) {
    throw new Error(`订单数量不能小于已退数量 ${formatMoney(returnedQuantity)}`)
  }

  const itemNo = orderForm.itemNo.trim() || generateOrderItemNo(data.products.find((product) => product.id === orderForm.productId) ?? undefined)
  const existingOrder = data.orders.find((order) => order.id === id)

  return {
    id,
    orderNo: orderForm.orderNo.trim() || existingOrder?.orderNo || generateOrderNo(itemNo),
    productId: orderForm.productId,
    customerId: orderForm.customerId,
    month: monthFromDate(orderTime),
    productName: orderForm.productName.trim(),
    itemNo,
    orderTime,
    deliveryDate,
    customerUnit: orderForm.customerUnit.trim(),
    customerName: orderForm.customerName.trim(),
    brand: orderForm.brand.trim(),
    unit: orderForm.unit.trim() || '件',
    catalogPrice: parseNonNegative('目录价', orderForm.catalogPrice),
    quantity,
    invoiceTotal: parseNonNegative('开票总价', orderForm.invoiceTotal),
    invoiceStatus: existingOrder?.invoiceStatus || orderForm.invoiceStatus || '未开票',
    isShipped: orderForm.isShipped,
    cashback: parseNonNegative('返现', orderForm.cashback),
    costDiscount: parseNonNegative('成本折扣', orderForm.costDiscount),
    costUnitPrice: parseNonNegative('成本单价', orderForm.costUnitPrice),
    costTotal: parseNonNegative('成本总价', orderForm.costTotal),
    saleDiscount: parseNonNegative('售价折扣', orderForm.saleDiscount),
    invoiceUnitPrice: parseNonNegative('开票单价', orderForm.invoiceUnitPrice),
    grossProfit: parseAmount(orderForm.grossProfit),
    remark: orderForm.remark.trim(),
    invoiceNo: existingOrder?.invoiceNo ?? '',
    isPaid: existingOrder?.isPaid ?? false,
    paidTime: normalizeDate(existingOrder?.paidTime ?? '') || '',
    returnedQuantity,
    createdAt,
  }
}

function saveOrder(): void {
  safeAction(() => {
    if (editingOrderId.value) {
      const existing = data.orders.find((order) => order.id === editingOrderId.value)
      if (!existing) {
        throw new Error('要编辑的订单不存在')
      }
      const next = buildOrder(existing.id, existing.returnedQuantity, existing.createdAt)
      data.orders.splice(data.orders.indexOf(existing), 1, next)
      syncInvoiceBackfills()
      editingOrderId.value = null
      Object.assign(orderForm, blankOrderForm())
      orderDialogVisible.value = false
      markSaved('出库单已更新')
      return
    }

    data.orders.push(buildOrder(createId(), 0, nowString()))
    Object.assign(orderForm, blankOrderForm())
    orderDialogVisible.value = false
    markSaved('出库单已新增')
  })
}

function openOrderCreate(): void {
  editingOrderId.value = null
  Object.assign(orderForm, blankOrderForm())
  orderDialogVisible.value = true
}

function editOrder(order: Order): void {
  Object.assign(orderForm, {
    orderNo: order.orderNo,
    productId: order.productId,
    customerId: order.customerId ?? findCustomerIdByText(data.customers, order.customerUnit, order.customerName),
    month: order.month || monthFromDate(order.orderTime),
    productName: order.productName,
    itemNo: order.itemNo,
    orderTime: order.orderTime,
    deliveryDate: order.deliveryDate,
    customerUnit: order.customerUnit,
    customerName: order.customerName,
    brand: order.brand,
    unit: order.unit,
    catalogPrice: formatMoney(order.catalogPrice),
    quantity: formatMoney(order.quantity),
    invoiceTotal: formatMoney(order.invoiceTotal),
    invoiceStatus: order.invoiceStatus,
    isShipped: order.isShipped,
    cashback: formatMoney(order.cashback),
    costDiscount: formatMoney(order.costDiscount),
    costUnitPrice: formatMoney(order.costUnitPrice),
    costTotal: formatMoney(order.costTotal),
    saleDiscount: formatMoney(order.saleDiscount),
    invoiceUnitPrice: formatMoney(order.invoiceUnitPrice),
    grossProfit: formatMoney(order.grossProfit),
    remark: order.remark,
    invoiceNo: order.invoiceNo,
    isPaid: order.isPaid,
    paidTime: order.paidTime,
  })
  editingOrderId.value = order.id
  orderDialogVisible.value = true
  activePage.value = 'orders'
  statusMessage.value = `正在编辑订单：${order.productName}`
}

async function deleteOrder(order: Order): Promise<void> {
  if (data.returns.some((record) => record.sourceOrderId === order.id)) {
    ElMessage.warning('该订单已有退货记录')
    return
  }
  if (data.invoices.some((invoice) => invoice.lines.some((line) => line.orderId === order.id))) {
    ElMessage.warning('该出库单已关联发票')
    return
  }
  if (!(await confirmDanger(`确认删除订单“${order.productName} / ${order.customerUnit}”？`, '删除订单'))) {
    return
  }
  data.orders = data.orders.filter((item) => item.id !== order.id)
  markSaved('订单已删除')
}

function invoiceLineFromOrder(order: Order): InvoiceLine {
  const invoicedAmount = data.invoices.reduce(
    (sum, invoice) => sum + invoice.lines.filter((line) => line.orderId === order.id).reduce((lineSum, line) => lineSum + Number(line.invoiceAmount || 0), 0),
    0,
  )
  const remainingAmount = Math.max(order.invoiceTotal - invoicedAmount, 0)
  return {
    id: createId(),
    orderId: order.id,
    orderNo: order.orderNo,
    productName: order.productName,
    itemNo: order.itemNo,
    customerUnit: order.customerUnit,
    customerName: order.customerName,
    orderAmount: order.invoiceTotal,
    invoiceAmount: remainingAmount || order.invoiceTotal,
  }
}

function invoiceLineFromImportedOrder(order: Order): InvoiceLine {
  return {
    id: createId(),
    orderId: order.id,
    orderNo: order.orderNo,
    productName: order.productName,
    itemNo: order.itemNo,
    customerUnit: order.customerUnit,
    customerName: order.customerName,
    orderAmount: order.invoiceTotal,
    invoiceAmount: order.invoiceTotal,
  }
}

function isImportInvoiceStatusInvoiced(value: string | undefined): boolean {
  const text = String(value ?? '').trim()
  return ['已开票', '全部开票', '部分开票'].includes(text)
}

function upsertInvoiceFromImportedOrder(order: Order): boolean {
  const invoiceNo = order.invoiceNo.trim()
  if (!invoiceNo || !isImportInvoiceStatusInvoiced(order.invoiceStatus)) {
    return false
  }

  const existing = data.invoices.find((invoice) => identity(invoice.invoiceNo) === identity(invoiceNo))
  if (existing) {
    if (!existing.lines.some((line) => line.orderId === order.id)) {
      existing.lines.push(invoiceLineFromImportedOrder(order))
    }
    existing.totalAmount = existing.lines.reduce((sum, line) => sum + Number(line.invoiceAmount || 0), 0)
    existing.isPaid = existing.isPaid || order.isPaid
    existing.paidTime = existing.isPaid ? [existing.paidTime, order.paidTime].filter(Boolean).sort().at(-1) ?? '' : ''
    return false
  }

  data.invoices.push({
    id: createId(),
    invoiceNo,
    invoiceDate: order.orderTime || todayString(),
    isPaid: order.isPaid,
    paidTime: order.isPaid ? order.paidTime || todayString() : '',
    totalAmount: order.invoiceTotal,
    remark: '出库单导入自动创建',
    lines: [invoiceLineFromImportedOrder(order)],
    createdAt: nowString(),
  })
  return true
}

function formatInvoiceLineSummary(lines: InvoiceLine[]): string {
  return [...new Set(lines.map((line) => line.orderNo).filter(Boolean))].join('；')
}

function openInvoiceCreateFromSelectedOrders(): void {
  if (!selectedOrders.value.length) {
    ElMessage.warning('请先选择要开票的出库单')
    return
  }
  Object.assign(invoiceForm, blankInvoiceForm())
  invoiceForm.lines = selectedOrders.value.map(invoiceLineFromOrder)
  editingInvoiceId.value = null
  invoiceDialogVisible.value = true
  activePage.value = 'invoices'
}

function editInvoice(invoice: Invoice): void {
  Object.assign(invoiceForm, {
    invoiceNo: invoice.invoiceNo,
    invoiceDate: invoice.invoiceDate,
    isPaid: invoice.isPaid,
    paidTime: invoice.paidTime,
    remark: invoice.remark,
    lines: invoice.lines.map((line) => ({ ...line })),
  })
  editingInvoiceId.value = invoice.id
  invoiceDialogVisible.value = true
  activePage.value = 'invoices'
}

function removeInvoiceLine(lineId: string): void {
  if (invoiceForm.lines.length <= 1) {
    ElMessage.warning('至少保留一条出库单明细')
    return
  }
  invoiceForm.lines = invoiceForm.lines.filter((line) => line.id !== lineId)
}

function buildInvoice(id: string, createdAt: string): Invoice {
  const invoiceNo = invoiceForm.invoiceNo.trim()
  const invoiceDate = normalizeDate(invoiceForm.invoiceDate)
  if (!invoiceNo) {
    throw new Error('发票号不能为空')
  }
  if (!invoiceDate) {
    throw new Error('开票日期不能为空')
  }
  if (!invoiceForm.lines.length) {
    throw new Error('请至少选择一张出库单')
  }
  const lines = invoiceForm.lines.map((line, index) => {
    const invoiceAmount = Number(line.invoiceAmount)
    if (!Number.isFinite(invoiceAmount) || invoiceAmount < 0) {
      throw new Error(`第 ${index + 1} 行发票金额无效`)
    }
    return {
      ...line,
      invoiceAmount,
    }
  })
  const totalAmount = lines.reduce((sum, line) => sum + line.invoiceAmount, 0)
  const paidTime = invoiceForm.isPaid ? normalizeDate(invoiceForm.paidTime) || todayString() : ''
  return {
    id,
    invoiceNo,
    invoiceDate,
    isPaid: invoiceForm.isPaid,
    paidTime,
    totalAmount,
    remark: invoiceForm.remark.trim(),
    lines,
    createdAt,
  }
}

function saveInvoice(): void {
  safeAction(() => {
    if (editingInvoiceId.value) {
      const existing = data.invoices.find((invoice) => invoice.id === editingInvoiceId.value)
      if (!existing) {
        throw new Error('要编辑的发票不存在')
      }
      data.invoices.splice(data.invoices.indexOf(existing), 1, buildInvoice(existing.id, existing.createdAt))
      syncInvoiceBackfills()
      resetInvoiceForm()
      markSaved('发票已更新，出库单状态已同步')
      return
    }

    data.invoices.push(buildInvoice(createId(), nowString()))
    syncInvoiceBackfills()
    resetInvoiceForm()
    markSaved('发票已生成，出库单状态已同步')
  })
}

async function deleteInvoice(invoice: Invoice): Promise<void> {
  if (!(await confirmDanger(`确认删除发票“${invoice.invoiceNo}”？`, '删除发票'))) {
    return
  }
  data.invoices = data.invoices.filter((item) => item.id !== invoice.id)
  syncInvoiceBackfills()
  markSaved('发票已删除，出库单状态已同步')
}

function resetInvoiceForm(): void {
  editingInvoiceId.value = null
  Object.assign(invoiceForm, blankInvoiceForm())
  invoiceForm.lines = []
  invoiceDialogVisible.value = false
}

function handleOrderSelectionChange(selection: Order[]): void {
  selectedOrders.value = selection
}

function printOrder(order: Order): void {
  void printOrders([order])
}

function printSelectedOrders(): void {
  if (!selectedOrders.value.length) {
    ElMessage.warning('请先选择要打印的订单')
    return
  }
  void printOrders(selectedOrders.value)
}

async function printOrders(orders: Order[]): Promise<void> {
  if (isPrinting.value) {
    ElMessage.warning('正在处理上一份出库单打印')
    return
  }
  isPrinting.value = true
  try {
    if (isTauriRuntime()) {
      printOrdersInCurrentWindow(orders)
      return
    }
    printOrdersInBrowser(orders)
  } finally {
    isPrinting.value = false
  }
}

function printOrdersInBrowser(orders: Order[]): void {
  const printWindow = window.open('', '_blank', 'popup=yes,width=900,height=700')
  const printDocument = printWindow?.document
  if (!printDocument || !printWindow) {
    printOrdersInCurrentWindow(orders)
    return
  }

  const cleanup = () => {
    setTimeout(() => {
      printWindow.close()
    }, 300)
  }

  const previousTitle = document.title
  document.title = '\u200B'
  printDocument.open()
  printDocument.write(buildDeliveryPrintHtml(orders))
  printDocument.close()
  printDocument.title = '\u200B'

  printWindow.addEventListener('afterprint', () => {
    document.title = previousTitle
    cleanup()
  }, { once: true })
  setTimeout(() => {
    printWindow.focus()
    printWindow.print()
    setTimeout(() => {
      document.title = previousTitle
      cleanup()
    }, 5000)
  }, 300)
  statusMessage.value = `已发送 ${orders.length} 条明细到 1 张出库单打印`
}

function printOrdersInCurrentWindow(orders: Order[]): void {
  const profile = activePrintProfile.value
  const { root, style } = mountBrowserPrintContent(orders, profile)
  const cleanup = () => {
    setTimeout(() => {
      root.remove()
      style.remove()
    }, 300)
  }

  const previousTitle = document.title
  document.title = '\u200B'
  window.addEventListener('afterprint', () => {
    document.title = previousTitle
    cleanup()
  }, { once: true })

  void nextTick()
    .then(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())))
    .then(() => {
      window.print()
      setTimeout(() => {
        document.title = previousTitle
        cleanup()
      }, 5000)
      statusMessage.value = `已发送 ${orders.length} 条明细到浏览器打印`
    })
}

function mountBrowserPrintContent(orders: Order[], profile: PrintProfile): { root: HTMLDivElement; style: HTMLStyleElement } {
  const root = document.createElement('div')
  root.id = 'browser-delivery-print-root'
  root.innerHTML = buildDeliveryPrintPage(orders, profile, printContentSettings)

  const style = document.createElement('style')
  style.dataset.browserDeliveryPrint = 'true'
  style.textContent = `
    #browser-delivery-print-root { display: none; }
    @page { size: ${profile.pageSize}; margin: 0; }
    @media print {
      @page { size: ${profile.pageSize}; margin: 0; }
      html,
      body {
        margin: 0 !important;
        padding: 0 !important;
        background: #fff !important;
      }
      body > *:not(#browser-delivery-print-root) {
        display: none !important;
      }
      #browser-delivery-print-root {
        display: block !important;
        color: #000 !important;
        background: #fff !important;
        font-family: "SimSun", "宋体", serif !important;
      }
      #browser-delivery-print-root * { box-sizing: border-box; }
      #browser-delivery-print-root .sheet {
        width: ${profile.sheetWidth};
        max-width: 100%;
        min-height: ${profile.sheetMinHeight};
        margin: 0 auto;
        page-break-after: always;
        padding: ${profile.margin} 0;
        overflow: hidden;
      }
      #browser-delivery-print-root .sheet:last-child { page-break-after: auto; }
      #browser-delivery-print-root .delivery-title {
        position: relative;
        padding: 0 4mm 2mm;
        text-align: center;
      }
      #browser-delivery-print-root .delivery-title h1 {
        margin: 0;
        font-size: ${profile.titleSize};
        line-height: 1.38;
        font-weight: 700;
        letter-spacing: 0;
      }
      #browser-delivery-print-root .delivery-title h2 {
        margin: 1mm 0 0;
        font-size: ${profile.subtitleSize};
        line-height: 1.35;
        font-weight: 700;
        letter-spacing: 0;
      }
      #browser-delivery-print-root .order-no {
        position: absolute;
        right: 1mm;
        bottom: 3mm;
        font-size: ${profile.orderNoSize};
        font-weight: 700;
      }
      #browser-delivery-print-root table {
        width: 100%;
        border-collapse: collapse;
        table-layout: fixed;
        font-size: ${profile.tableFontSize};
      }
      #browser-delivery-print-root td,
      #browser-delivery-print-root th {
        border: 1px solid #000;
        height: ${profile.cellHeight};
        padding: 1.2mm 1.5mm;
        text-align: center;
        vertical-align: middle;
        font-weight: 400;
        overflow-wrap: anywhere;
      }
      #browser-delivery-print-root .label {
        width: 23mm;
        font-weight: 400;
      }
      #browser-delivery-print-root .head-cell {
        font-size: ${profile.tableFontSize};
        font-weight: 400;
      }
      #browser-delivery-print-root .text-left { text-align: left; }
      #browser-delivery-print-root .money {
        text-align: right;
        padding-right: 3mm;
        font-family: "Courier New", monospace;
      }
      #browser-delivery-print-root .item-row td { height: ${profile.itemRowHeight}; }
      #browser-delivery-print-root .blank-row td { height: ${profile.blankRowHeight}; }
      #browser-delivery-print-root .footer td { height: ${profile.cellHeight}; }
      #browser-delivery-print-root .signature {
        height: ${profile.signatureHeight};
        text-align: center;
        line-height: 1.8;
        font-size: ${profile.tableFontSize};
      }
    }
  `

  document.head.appendChild(style)
  document.body.appendChild(root)
  return { root, style }
}

function buildDeliveryPrintHtml(orders: Order[]): string {
  const profile = activePrintProfile.value
  const page = buildDeliveryPrintPage(orders, profile, printContentSettings)
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>&#8203;</title>
  <style>
    @page { size: ${profile.pageSize}; margin: 0; }
    @media print {
      @page { size: ${profile.pageSize}; margin: 0 !important; }
      html, body { margin: 0 !important; padding: 0 !important; }
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      color: #000;
      background: #fff;
      font-family: "SimSun", "宋体", serif;
    }
    .sheet {
      width: ${profile.sheetWidth};
      max-width: 100%;
      min-height: ${profile.sheetMinHeight};
      margin: 0 auto;
      page-break-after: always;
      padding: ${profile.margin} 0;
      overflow: hidden;
    }
    .sheet:last-child { page-break-after: auto; }
    .delivery-title {
      position: relative;
      padding: 0 4mm 2mm;
      text-align: center;
    }
    .delivery-title h1 {
      margin: 0;
      font-size: ${profile.titleSize};
      line-height: 1.38;
      font-weight: 700;
      letter-spacing: 0;
    }
    .delivery-title h2 {
      margin: 1mm 0 0;
      font-size: ${profile.subtitleSize};
      line-height: 1.35;
      font-weight: 700;
      letter-spacing: 0;
    }
    .order-no {
      position: absolute;
      right: 1mm;
      bottom: 3mm;
      font-size: ${profile.orderNoSize};
      font-weight: 700;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      table-layout: fixed;
      font-size: ${profile.tableFontSize};
    }
    td, th {
      border: 1px solid #000;
      height: ${profile.cellHeight};
      padding: 1.2mm 1.5mm;
      text-align: center;
      vertical-align: middle;
      font-weight: 400;
      overflow-wrap: anywhere;
    }
    .label {
      width: 23mm;
      font-weight: 400;
    }
    .head-cell {
      font-size: ${profile.tableFontSize};
      font-weight: 400;
    }
    .text-left { text-align: left; }
    .money { text-align: right; padding-right: 3mm; font-family: "Courier New", monospace; }
    .item-row td { height: ${profile.itemRowHeight}; }
    .blank-row td { height: ${profile.blankRowHeight}; }
    .footer td { height: ${profile.cellHeight}; }
    .signature {
      height: ${profile.signatureHeight};
      text-align: center;
      line-height: 1.8;
      font-size: ${profile.tableFontSize};
    }
  </style>
</head>
<body>${page}</body>
</html>`
}

function buildDeliveryPrintPage(orders: Order[], profile: PrintProfile, settings: PrintContentSettings): string {
  const firstOrder = orders[0]
  const rows = orders.map((order) => buildDeliveryPrintItemRow(order, settings)).join('')
  const amount = orders.reduce((sum, order) => sum + order.invoiceTotal, 0)
  const blankRowCount = Math.max(profile.blankRows - orders.length + 1, 0)
  const blanks = Array.from({ length: blankRowCount }, () => `
    <tr class="blank-row">
      <td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td>
    </tr>`).join('')

  return `
  <section class="sheet">
    <div class="delivery-title">
      <h1>${escapeHtml(settings.companyName)}</h1>
      <h2>${escapeHtml(settings.documentTitle)}</h2>
      <div class="order-no">${escapeHtml(settings.orderNoPrefix)}${escapeHtml(buildPrintOrderNo(firstOrder))}</div>
    </div>
    <table>
      <colgroup>
        <col style="width: 8.5%;">
        <col style="width: 11.3%;">
        <col style="width: 22%;">
        <col style="width: 7%;">
        <col style="width: 7%;">
        <col style="width: 7%;">
        <col style="width: 7%;">
        <col style="width: 7%;">
        <col style="width: 13.2%;">
        <col style="width: 10%;">
      </colgroup>
      <tr>
        <td class="label">单位</td>
        <td colspan="2">${escapeHtml(firstOrder.customerUnit || '')}</td>
        <td colspan="2">开单日期</td>
        <td colspan="5">${escapeHtml(formatPrintDate(firstOrder.deliveryDate || firstOrder.orderTime))}</td>
      </tr>
      <tr>
        <td class="label">订货人</td>
        <td colspan="2">${escapeHtml(firstOrder.customerName || '')}</td>
        <td colspan="7">地址</td>
      </tr>
      <tr>
        <td class="head-cell">品牌</td>
        <td class="head-cell">货号</td>
        <td class="head-cell">品名规格</td>
        <td class="head-cell">单位</td>
        <td class="head-cell">数量</td>
        <td class="head-cell">批号</td>
        <td class="head-cell">出货仓</td>
        <td class="head-cell">温度</td>
        <td class="head-cell">单价</td>
        <td class="head-cell">总额</td>
      </tr>
      ${rows}
      ${blanks}
      <tr class="footer">
        <td colspan="2">合计</td>
        <td colspan="2">人民币（大写）：</td>
        <td colspan="3">${escapeHtml(moneyToChinese(amount))}</td>
        <td></td>
        <td></td>
        <td class="money">${formatMoney(amount)}</td>
      </tr>
      <tr>
        <td colspan="3" class="signature">  
          <div>${escapeHtml(settings.receiverSignatureLabel)}</div>
          <div>${escapeHtml(settings.receiverSignatureHint)}</div>
        </td>
        <td colspan="7"></td>
      </tr>
    </table>
  </section>`
}

function buildDeliveryPrintItemRow(order: Order, settings: PrintContentSettings): string {
  const product = data.products.find((item) => item.id === order.productId)
  const itemName = [order.productName, product?.specification].filter(Boolean).join(' ')
  return `
    <tr class="item-row">
      <td>${escapeHtml(order.brand || '-')}</td>
      <td>${escapeHtml(order.itemNo || '-')}</td>
      <td>${escapeHtml(itemName || '-')}</td>
      <td>${escapeHtml(order.unit || '-')}</td>
      <td>${formatMoney(order.quantity)}</td>
      <td></td>
      <td>${escapeHtml(settings.warehouseName)}</td>
      <td></td>
      <td class="money">${formatMoney(order.invoiceUnitPrice)}</td>
      <td class="money">${formatMoney(order.invoiceTotal)}</td>
    </tr>`
}

function buildPrintOrderNo(order: Order): string {
  const dateText = (normalizeDate(order.deliveryDate) || normalizeDate(order.orderTime) || todayString()).replaceAll('-', '')
  return `000-${dateText}1`
}

function formatPrintDate(value: string): string {
  const normalized = normalizeDate(value) || todayString()
  const [year, month, day] = normalized.split('-')
  return `${Number(year)}/${Number(month)}/${Number(day)}`
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

const CHINESE_DIGITS = ['零', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖']
const CHINESE_SECTION_UNITS = ['', '万', '亿']
const CHINESE_PLACE_UNITS = ['', '拾', '佰', '仟']

function moneyToChinese(value: number): string {
  const amount = Math.round(Math.abs(value) * 100)
  const yuan = Math.floor(amount / 100)
  const jiao = Math.floor((amount % 100) / 10)
  const fen = amount % 10
  const suffix = value < 0 ? '负' : ''
  if (yuan === 0 && jiao === 0 && fen === 0) {
    return '零元整'
  }

  let result = yuan > 0 ? `${integerToChinese(yuan)}元` : ''
  if (jiao > 0) {
    result += `${CHINESE_DIGITS[jiao]}角`
  }
  if (fen > 0) {
    if (jiao === 0 && yuan > 0) {
      result += '零'
    }
    result += `${CHINESE_DIGITS[fen]}分`
  }
  if (jiao === 0 && fen === 0) {
    result += '整'
  }
  return `${suffix}${result}`
}

function integerToChinese(value: number): string {
  if (value === 0) {
    return '零'
  }

  const sections: string[] = []
  let rest = value
  while (rest > 0) {
    sections.push(sectionToChinese(rest % 10000))
    rest = Math.floor(rest / 10000)
  }

  let result = ''
  let needZero = false
  for (let index = sections.length - 1; index >= 0; index -= 1) {
    const section = sections[index]
    if (!section) {
      needZero = result.length > 0
      continue
    }
    if (needZero || (result && section.length < 4)) {
      result += '零'
    }
    result += `${section}${CHINESE_SECTION_UNITS[index]}`
    needZero = false
  }
  return result.replace(/零+/g, '零').replace(/零$/g, '')
}

function sectionToChinese(value: number): string {
  let result = ''
  let zero = false
  for (let unitIndex = 0; unitIndex < 4; unitIndex += 1) {
    const digit = value % 10
    if (digit === 0) {
      zero = result.length > 0
    } else {
      result = `${zero ? '零' : ''}${CHINESE_DIGITS[digit]}${CHINESE_PLACE_UNITS[unitIndex]}${result}`
      zero = false
    }
    value = Math.floor(value / 10)
  }
  return result
}

function fillReturnFromOrder(orderId: string): void {
  const order = data.orders.find((item) => item.id === orderId)
  if (!order) {
    return
  }
  const quantity = Math.min(1, currentOutboundQuantity(order))
  Object.assign(returnForm, {
    sourceOrderId: order.id,
    productId: order.productId,
    productName: order.productName,
    itemNo: order.itemNo,
    returnTime: todayString(),
    customerUnit: order.customerUnit,
    customerName: order.customerName,
    brand: order.brand,
    unit: order.unit,
    catalogPrice: formatMoney(order.catalogPrice),
    quantity: formatMoney(quantity),
    invoiceTotal: formatMoney(proratedAmount(order.invoiceTotal, order.quantity, quantity)),
    cashback: '0.00',
    costDiscount: formatMoney(order.costDiscount),
    costUnitPrice: formatMoney(order.costUnitPrice),
    costTotal: formatMoney(order.costUnitPrice * quantity),
    saleDiscount: formatMoney(order.saleDiscount),
    invoiceUnitPrice: formatMoney(order.invoiceUnitPrice),
    grossProfit: formatMoney((order.invoiceUnitPrice - order.costUnitPrice) * quantity),
    remark: '',
    paidTime: todayString(),
  })
}

function prepareReturn(order: Order): void {
  Object.assign(returnForm, blankReturnForm())
  fillReturnFromOrder(order.id)
  editingReturnId.value = null
  returnDialogVisible.value = true
  activePage.value = 'returns'
}

function recalculateReturnAmounts(showMessage = true): void {
  safeAction(() => {
    const quantity = parsePositive('数量', returnForm.quantity)
    const invoiceUnitPrice = parseNonNegative('销售单价', returnForm.invoiceUnitPrice)
    const costUnitPrice = parseNonNegative('成本单价', returnForm.costUnitPrice)
    const cashback = parseNonNegative('返现', returnForm.cashback)
    const invoiceTotal = quantity * invoiceUnitPrice
    const costTotal = quantity * costUnitPrice
    returnForm.invoiceTotal = formatMoney(invoiceTotal)
    returnForm.costTotal = formatMoney(costTotal)
    returnForm.grossProfit = formatMoney(invoiceTotal - costTotal - cashback)
    if (showMessage) {
      statusMessage.value = '退货金额已重算'
    }
  })
}

function buildReturn(id: string, createdAt: string): ReturnRecord {
  syncReturnAmounts()
  const sourceOrder = data.orders.find((order) => order.id === returnForm.sourceOrderId)
  if (!sourceOrder) {
    throw new Error('请先关联订单')
  }
  const returnTime = normalizeDate(returnForm.returnTime)
  if (!returnTime) {
    throw new Error('退货日期不能为空')
  }
  if (dateValue(returnTime) < dateValue(sourceOrder.orderTime)) {
    throw new Error('退货日期不能早于订购日期')
  }

  const quantity = parsePositive('数量', returnForm.quantity)
  const alreadyReturned = data.returns
    .filter((record) => record.sourceOrderId === sourceOrder.id && record.id !== editingReturnId.value)
    .reduce((sum, record) => sum + record.quantity, 0)
  if (alreadyReturned + quantity - sourceOrder.quantity > Number.EPSILON) {
    throw new Error(`退货数量不能超过可退数量 ${formatMoney(sourceOrder.quantity - alreadyReturned)}`)
  }

  return {
    id,
    sourceOrderId: sourceOrder.id,
    productId: returnForm.productId,
    productName: returnForm.productName.trim() || sourceOrder.productName,
    itemNo: returnForm.itemNo.trim(),
    returnTime,
    customerUnit: returnForm.customerUnit.trim(),
    customerName: returnForm.customerName.trim(),
    brand: returnForm.brand.trim(),
    unit: returnForm.unit.trim() || '件',
    catalogPrice: parseNonNegative('目录价', returnForm.catalogPrice),
    quantity,
    invoiceTotal: parseNonNegative('销售总价', returnForm.invoiceTotal),
    cashback: parseNonNegative('返现', returnForm.cashback),
    costDiscount: parseNonNegative('成本折扣', returnForm.costDiscount),
    costUnitPrice: parseNonNegative('成本单价', returnForm.costUnitPrice),
    costTotal: parseNonNegative('成本总价', returnForm.costTotal),
    saleDiscount: parseNonNegative('售价折扣', returnForm.saleDiscount),
    invoiceUnitPrice: parseNonNegative('销售单价', returnForm.invoiceUnitPrice),
    grossProfit: parseAmount(returnForm.grossProfit),
    remark: returnForm.remark.trim(),
    paidTime: normalizeDate(returnForm.paidTime) || todayString(),
    createdAt,
  }
}

function saveReturn(): void {
  safeAction(() => {
    if (editingReturnId.value) {
      const existing = data.returns.find((record) => record.id === editingReturnId.value)
      if (!existing) {
        throw new Error('要编辑的退货记录不存在')
      }
      data.returns.splice(data.returns.indexOf(existing), 1, buildReturn(existing.id, existing.createdAt))
      editingReturnId.value = null
      Object.assign(returnForm, blankReturnForm())
      returnDialogVisible.value = false
      recalculateReturnedQuantities()
      markSaved('退货记录已更新')
      return
    }

    data.returns.push(buildReturn(createId(), nowString()))
    Object.assign(returnForm, blankReturnForm())
    returnDialogVisible.value = false
    recalculateReturnedQuantities()
    markSaved('退货记录已新增')
  })
}

function openReturnCreate(): void {
  editingReturnId.value = null
  Object.assign(returnForm, blankReturnForm())
  returnDialogVisible.value = true
}

function editReturn(record: ReturnRecord): void {
  Object.assign(returnForm, {
    sourceOrderId: record.sourceOrderId,
    productId: record.productId,
    productName: record.productName,
    itemNo: record.itemNo,
    returnTime: record.returnTime,
    customerUnit: record.customerUnit,
    customerName: record.customerName,
    brand: record.brand,
    unit: record.unit,
    catalogPrice: formatMoney(record.catalogPrice),
    quantity: formatMoney(record.quantity),
    invoiceTotal: formatMoney(record.invoiceTotal),
    cashback: formatMoney(record.cashback),
    costDiscount: formatMoney(record.costDiscount),
    costUnitPrice: formatMoney(record.costUnitPrice),
    costTotal: formatMoney(record.costTotal),
    saleDiscount: formatMoney(record.saleDiscount),
    invoiceUnitPrice: formatMoney(record.invoiceUnitPrice),
    grossProfit: formatMoney(record.grossProfit),
    remark: record.remark,
    paidTime: record.paidTime,
  })
  editingReturnId.value = record.id
  returnDialogVisible.value = true
  activePage.value = 'returns'
  statusMessage.value = `正在编辑退货：${record.productName}`
}

async function deleteReturn(record: ReturnRecord): Promise<void> {
  if (!(await confirmDanger(`确认删除退货记录“${record.productName}”？`, '删除退货'))) {
    return
  }
  data.returns = data.returns.filter((item) => item.id !== record.id)
  recalculateReturnedQuantities()
  markSaved('退货记录已删除')
}

async function confirmDanger(message: string, title: string): Promise<boolean> {
  try {
    await ElMessageBox.confirm(message, title, { type: 'warning' })
    return true
  } catch {
    return false
  }
}

function recalculateReturnedQuantities(): void {
  data.orders.forEach((order) => {
    order.returnedQuantity = data.returns
      .filter((record) => record.sourceOrderId === order.id)
      .reduce((sum, record) => sum + record.quantity, 0)
  })
}

function resetProductForm(): void {
  editingProductId.value = null
  Object.assign(productForm, blankProductForm())
  productDialogVisible.value = false
}

function resetCustomerForm(): void {
  editingCustomerId.value = null
  Object.assign(customerForm, blankCustomerForm())
  customerDialogVisible.value = false
}

function resetOrderForm(): void {
  editingOrderId.value = null
  Object.assign(orderForm, blankOrderForm())
  orderDialogVisible.value = false
}

function resetReturnForm(): void {
  editingReturnId.value = null
  Object.assign(returnForm, blankReturnForm())
  returnDialogVisible.value = false
}

function openImportDialog(type: ImportType): void {
  activeImportType.value = type
  importDialogVisible.value = true
}

function importTypeLabel(type: ImportType): string {
  if (type === 'products') {
    return '产品'
  }
  if (type === 'orders') {
    return '出库单'
  }
  if (type === 'invoices') {
    return '发票'
  }
  return '退货'
}

function triggerImportUpload(): void {
  if (activeImportType.value === 'products') {
    openFile(productFileInput.value)
    return
  }
  if (activeImportType.value === 'orders') {
    openFile(orderFileInput.value)
    return
  }
  openFile(returnFileInput.value)
}

function openFile(input: HTMLInputElement | null): void {
  if (!input) {
    return
  }
  input.value = ''
  input.click()
}

async function readSelectedFile(event: Event): Promise<string | null> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  return file ? file.text() : null
}

async function readSelectedExcelRows(event: Event): Promise<Record<string, string>[] | null> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) {
    return null
  }

  const workbook = XLSX.read(await file.arrayBuffer(), { type: 'array', cellDates: false })
  const firstSheetName = workbook.SheetNames[0]
  if (!firstSheetName) {
    return []
  }

  return XLSX.utils
    .sheet_to_json<Record<string, unknown>>(workbook.Sheets[firstSheetName], {
      defval: '',
      raw: false,
    })
    .map((row) => Object.fromEntries(Object.entries(row).map(([key, value]) => [key.trim(), String(value).trim()])))
}

async function importProductsExcel(event: Event): Promise<void> {
  const rows = await readSelectedExcelRows(event)
  if (!rows) {
    return
  }
  safeAction(() => {
    let imported = 0
    let skipped = 0
    for (const row of rows) {
      const name = row['产品名称']?.trim()
      if (!name) {
        continue
      }
      const duplicate = data.products.some(
        (product) =>
          identity(product.name) === identity(name)
          && identity(product.brand) === identity(row['品牌'] ?? '')
          && identity(product.specification) === identity(row['规格/型号'] ?? ''),
      )
      if (duplicate) {
        skipped += 1
        continue
      }
      data.products.push({
        id: createId(),
        name,
        purchasePrice: parseNonNegative('进货金额', row['进货金额'] ?? '0'),
        outboundPrice: parseNonNegative('出库金额', row['出库金额'] ?? '0'),
        specification: row['规格/型号'] ?? '',
        brand: row['品牌'] ?? '',
        unit: row['单位'] || '件',
        description: row['描述'] ?? '',
        createdAt: row['创建时间'] || nowString(),
      })
      imported += 1
    }
    markSaved(`产品Excel已导入 ${imported} 条，跳过重复 ${skipped} 条`)
    importDialogVisible.value = false
  })
}

async function importOrdersExcel(event: Event): Promise<void> {
  const rows = await readSelectedExcelRows(event)
  if (!rows) {
    return
  }
  safeAction(() => {
    let imported = 0
    let skipped = 0
    let createdProducts = 0
    let createdCustomers = 0
    let createdInvoices = 0
    let linkedInvoices = 0
    const importedOrderKeys = new Set(data.orders.map(orderImportKey))
    for (const row of rows) {
      const productName = (row['品名'] || row['产品名称'] || '').trim()
      if (!productName) {
        continue
      }
      const orderTime = normalizeDate(row['订购时间'] ?? '') || todayString()
      const brand = (row['品牌'] ?? '').trim()
      const unit = (row['单位'] ?? '').trim() || '件'
      const rawItemNo = (row['货号'] ?? '').trim()
      const existingProduct = findProductForOrderImport(productName, brand, rawItemNo)
      const itemNo = rawItemNo || existingProduct?.specification || ''
      const orderNoPrefix = itemNo || productName
      const invoiceUnitPrice = parseNonNegative('开票单价', row['开票单价'] ?? row['销售单价'] ?? '0')
      const costUnitPrice = parseNonNegative('成本单价', row['成本单价'] ?? '0')
      const quantity = parsePositive('数量', row['数量'] ?? '0')
      const cashback = parseImportCashback(row['返现'])
      const invoiceTotal = quantity * invoiceUnitPrice
      const costTotal = quantity * costUnitPrice
      const grossProfit = invoiceTotal - costTotal - cashback
      const orderMonth = normalizeImportMonth(row['月份'], orderTime)
      const catalogPrice = parseNonNegative('目录价', row['目录价'] ?? '0')
      const costDiscount = parseNonNegative('成本折扣', row['成本折扣'] ?? '0')
      const saleDiscount = parseNonNegative('售价折扣', row['售价折扣'] ?? '0')
      const isShipped = parseYesNo(row['是否发货'], true)
      const returnedQuantity = parseNonNegative('已退数量', row['已退数量'] ?? '0')
      const remark = row['备注'] ?? ''
      const invoiceNo = row['发票号'] ?? ''
      const orderKey = orderImportKey({
        month: orderMonth,
        productName,
        itemNo,
        orderTime,
        customerUnit: row['订货单位'] ?? '',
        customerName: row['订货人'] ?? '',
        brand,
        unit,
        catalogPrice,
        quantity,
        cashback,
        costDiscount,
        costUnitPrice,
        saleDiscount,
        invoiceUnitPrice,
        remark,
        invoiceNo,
        isShipped,
        returnedQuantity,
      })
      if (importedOrderKeys.has(orderKey)) {
        skipped += 1
        continue
      }

      const product = ensureProductForOrderImport(productName, brand, itemNo, unit, costUnitPrice, invoiceUnitPrice)
      if (!existingProduct) {
        createdProducts += 1
      }

      const customerUnit = (row['订货单位'] ?? '').trim()
      const customerName = (row['订货人'] ?? '').trim()
      const existingCustomerId = findCustomerIdByText(data.customers, customerUnit, customerName)
      const customer = ensureCustomerForOrderImport(customerUnit, customerName)
      if (!existingCustomerId && customer) {
        createdCustomers += 1
      }

      const order: Order = {
        id: createId(),
        orderNo: generateOrderNo(orderNoPrefix),
        productId: product?.id ?? null,
        customerId: customer?.id ?? null,
        month: orderMonth,
        productName,
        itemNo,
        orderTime,
        deliveryDate: orderTime,
        customerUnit,
        customerName,
        brand,
        unit,
        catalogPrice,
        quantity,
        invoiceTotal,
        invoiceStatus: row['开票情况'] || '未开票',
        isShipped,
        cashback,
        costDiscount,
        costUnitPrice,
        costTotal,
        saleDiscount,
        invoiceUnitPrice,
        grossProfit,
        remark,
        invoiceNo,
        isPaid: parseYesNo(row['是否回款'], false),
        paidTime: normalizeDate(row['回款时间'] ?? '') || '',
        returnedQuantity,
        createdAt: row['创建时间'] || nowString(),
      }
      data.orders.push(order)
      importedOrderKeys.add(orderKey)
      if (upsertInvoiceFromImportedOrder(order)) {
        createdInvoices += 1
      } else if (order.invoiceNo.trim() && isImportInvoiceStatusInvoiced(order.invoiceStatus)) {
        linkedInvoices += 1
      }
      imported += 1
    }
    syncInvoiceBackfills()
    markSaved(`订单Excel已导入 ${imported} 条，跳过重复 ${skipped} 条，自动创建产品 ${createdProducts} 个、客户 ${createdCustomers} 个、发票 ${createdInvoices} 张、关联发票 ${linkedInvoices} 条`)
    importDialogVisible.value = false
  })
}

async function importReturnsExcel(event: Event): Promise<void> {
  const rows = await readSelectedExcelRows(event)
  if (!rows) {
    return
  }
  safeAction(() => {
    let imported = 0
    for (const row of rows) {
      const sourceOrderId = row['关联订单ID'] || ''
      const order = data.orders.find((item) => item.id === sourceOrderId)
      if (!order) {
        continue
      }
      data.returns.push({
        id: createId(),
        sourceOrderId: order.id,
        productId: order.productId,
        productName: row['产品名称'] || order.productName,
        itemNo: row['货号'] ?? '',
        returnTime: normalizeDate(row['退货日期'] ?? '') || todayString(),
        customerUnit: row['订货单位'] ?? '',
        customerName: row['订货人'] ?? '',
        brand: row['品牌'] ?? '',
        unit: row['单位'] || '件',
        catalogPrice: parseNonNegative('目录价', row['目录价'] ?? '0'),
        quantity: parsePositive('数量', row['数量'] ?? '0'),
        invoiceTotal: parseNonNegative('销售总价', row['销售总价'] ?? '0'),
        cashback: parseNonNegative('返现', row['返现'] ?? '0'),
        costDiscount: parseNonNegative('成本折扣', row['成本折扣'] ?? '0'),
        costUnitPrice: parseNonNegative('成本单价', row['成本单价'] ?? '0'),
        costTotal: parseNonNegative('成本总价', row['成本总价'] ?? '0'),
        saleDiscount: parseNonNegative('售价折扣', row['售价折扣'] ?? '0'),
        invoiceUnitPrice: parseNonNegative('销售单价', row['销售单价'] ?? '0'),
        grossProfit: parseAmount(row['毛利'] ?? '0'),
        remark: row['备注'] ?? '',
        paidTime: todayString(),
        createdAt: row['创建时间'] || nowString(),
      })
      imported += 1
    }
    recalculateReturnedQuantities()
    markSaved(`退货Excel已导入 ${imported} 条`)
    importDialogVisible.value = false
  })
}

async function exportTemplates(): Promise<void> {
  const workbook = XLSX.utils.book_new()
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet([PRODUCT_HEADERS]), '产品')
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet([ORDER_HEADERS]), '订单')
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet([RETURN_HEADERS]), '退货')
  const saved = await saveBinaryFile(
    'inventory-import-templates.xlsx',
    XLSX.write(workbook, { bookType: 'xlsx', type: 'array' }) as ArrayBuffer,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  )
  if (saved) {
    statusMessage.value = 'Excel模板已导出'
  }
}

async function exportImportTemplate(type: ImportType = activeImportType.value): Promise<void> {
  const workbook = XLSX.utils.book_new()
  const label = importTypeLabel(type)
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet([importTemplateHeaders(type)]), label)
  const saved = await saveBinaryFile(
    `${label}-导入模板.xlsx`,
    XLSX.write(workbook, { bookType: 'xlsx', type: 'array' }) as ArrayBuffer,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  )
  if (saved) {
    statusMessage.value = `${label}导入模板已下载`
  }
}

function importTemplateHeaders(type: ImportType): string[] {
  if (type === 'products') {
    return PRODUCT_HEADERS
  }
  if (type === 'orders') {
    return ORDER_HEADERS
  }
  if (type === 'invoices') {
    return INVOICE_HEADERS
  }
  return RETURN_HEADERS
}

async function exportProductsExcel(): Promise<void> {
  await exportSingleSheetExcel('产品', PRODUCT_HEADERS, productExportRows(), '产品.xlsx', '产品Excel已导出')
}

async function exportOrdersExcel(): Promise<void> {
  await exportSingleSheetExcel('出库单', ORDER_HEADERS, orderExportRows(), '出库单.xlsx', '出库单Excel已导出')
}

async function exportInvoicesExcel(): Promise<void> {
  await exportSingleSheetExcel('发票', INVOICE_HEADERS, invoiceExportRows(), '发票.xlsx', '发票Excel已导出')
}

async function exportReturnsExcel(): Promise<void> {
  await exportSingleSheetExcel('退货', RETURN_HEADERS, returnExportRows(), '退货单.xlsx', '退货Excel已导出')
}

async function exportSingleSheetExcel(
  sheetName: string,
  headers: string[],
  rows: Array<Array<string | number>>,
  filename: string,
  message: string,
): Promise<void> {
  const workbook = XLSX.utils.book_new()
  XLSX.utils.book_append_sheet(workbook, XLSX.utils.aoa_to_sheet([headers, ...rows]), sheetName)
  const saved = await saveBinaryFile(
    filename,
    XLSX.write(workbook, { bookType: 'xlsx', type: 'array' }) as ArrayBuffer,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  )
  if (saved) {
    statusMessage.value = message
  }
}

function productExportRows(): Array<Array<string | number>> {
  return data.products.map((product) => [
    product.name,
    formatMoney(product.purchasePrice),
    formatMoney(product.outboundPrice),
    product.specification,
    product.brand,
    product.unit || '件',
    product.description,
    product.createdAt,
  ])
}

function orderExportRows(): Array<Array<string | number>> {
  return data.orders.map((order) => [
    order.month || monthFromDate(order.orderTime),
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
    yesNo(order.isShipped),
    formatMoney(order.cashback),
    formatMoney(order.costDiscount),
    formatMoney(order.costUnitPrice),
    formatMoney(order.costTotal),
    formatMoney(order.saleDiscount),
    formatMoney(order.invoiceUnitPrice),
    formatMoney(order.grossProfit),
    order.remark,
    order.invoiceNo,
    yesNo(order.isPaid),
    order.paidTime,
  ])
}

function invoiceExportRows(): Array<Array<string | number>> {
  return data.invoices.map((invoice) => [
    invoice.invoiceNo,
    invoice.invoiceDate,
    formatMoney(invoice.totalAmount),
    yesNo(invoice.isPaid),
    invoice.paidTime,
    invoice.lines.map((line) => `${line.orderNo}:${formatMoney(line.invoiceAmount)}`).join('; '),
    invoice.remark,
    invoice.createdAt,
  ])
}

function returnExportRows(): Array<Array<string | number>> {
  return data.returns.map((record) => [
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
  ])
}

async function exportAllExcel(): Promise<void> {
  const workbook = XLSX.utils.book_new()
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      PRODUCT_HEADERS,
      ...productExportRows(),
    ]),
    '产品',
  )
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      ORDER_HEADERS,
      ...orderExportRows(),
    ]),
    '出库单',
  )
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      INVOICE_HEADERS,
      ...invoiceExportRows(),
    ]),
    '发票',
  )
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      RETURN_HEADERS,
      ...returnExportRows(),
    ]),
    '退货',
  )
  const saved = await saveBinaryFile(
    'inventory-export.xlsx',
    XLSX.write(workbook, { bookType: 'xlsx', type: 'array' }) as ArrayBuffer,
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  )
  if (saved) {
    statusMessage.value = 'Excel已导出'
  }
}

function exportBackup(): void {
  downloadText('inventory-backup.json', JSON.stringify(data, null, 2), 'application/json;charset=utf-8')
  statusMessage.value = '数据备份已导出'
}

async function saveBinaryFile(filename: string, content: BlobPart, type: string): Promise<boolean> {
  if (isTauriRuntime()) {
    try {
      const [{ save }, { writeFile }] = await Promise.all([
        import('@tauri-apps/plugin-dialog'),
        import('@tauri-apps/plugin-fs'),
      ])
      const filePath = await save({
        defaultPath: filename,
        filters: [{ name: 'Excel', extensions: ['xlsx'] }],
      })
      if (!filePath) {
        return false
      }
      const bytes = content instanceof ArrayBuffer
        ? new Uint8Array(content)
        : new Uint8Array(await new Blob([content], { type }).arrayBuffer())
      await writeFile(filePath, bytes)
      return true
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      ElMessage.error(`文件保存失败：${message}`)
      return false
    }
  }

  downloadBinary(filename, content, type)
  return true
}

function isTauriRuntime(): boolean {
  return Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
}

async function writeAutoBackup(): Promise<void> {
  const content = JSON.stringify(data, null, 2)
  localStorage.setItem(AUTO_BACKUP_STORAGE_KEY, content)

  if (!isTauriRuntime()) {
    return
  }

  try {
    const { BaseDirectory, writeFile } = await import('@tauri-apps/plugin-fs')
    await writeFile(AUTO_BACKUP_FILENAME, new TextEncoder().encode(content), {
      baseDir: BaseDirectory.AppLocalData,
    })
  } catch (error) {
    console.warn('auto backup failed', error)
  }
}

function downloadBinary(filename: string, content: BlobPart, type: string): void {
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

async function importBackupJson(event: Event): Promise<void> {
  const content = await readSelectedFile(event)
  if (!content) {
    return
  }
  if (!(await confirmDanger('恢复备份会覆盖当前浏览器里的本地数据。确认继续？', '恢复JSON备份'))) {
    return
  }
  safeAction(() => {
    const parsed = JSON.parse(content) as Partial<AppData>
    data.version = Number(parsed.version || 1)
    data.products = Array.isArray(parsed.products)
      ? parsed.products.map((product) => ({ ...product, unit: product.unit || '件' }))
      : []
    data.customers = Array.isArray(parsed.customers) ? parsed.customers : []
    data.orders = Array.isArray(parsed.orders) ? parsed.orders : []
    data.orders = data.orders.map((order) => ({
      ...order,
      customerId: order.customerId ?? findCustomerIdByText(data.customers, order.customerUnit, order.customerName),
    }))
    data.returns = Array.isArray(parsed.returns) ? parsed.returns : []
    recalculateReturnedQuantities()
    markSaved('JSON备份已恢复')
  })
}

function setDashboardRange(type: 'today' | 'week' | 'month'): void {
  const today = todayString()
  dashboardFilter.endDate = today
  if (type === 'today') {
    dashboardFilter.startDate = today
  } else if (type === 'week') {
    dashboardFilter.startDate = addDays(today, -6)
  } else {
    dashboardFilter.startDate = `${today.slice(0, 8)}01`
  }
}

function percentOf(value: number, base: number): number {
  return Math.abs(base) > 0 ? value / base : 0
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`
}

function formatChartMoney(value: number): string {
  const sign = value < 0 ? '-' : ''
  const amount = Math.abs(value)
  if (amount >= 10000) {
    return `${sign}¥${(amount / 10000).toFixed(1)}万`
  }
  return `${sign}¥${formatMoney(amount)}`
}

function chartValueY(value: number): number {
  const scaleRange = chartScaleMax.value - chartScaleMin.value
  const ratio = scaleRange > 0 ? (value - chartScaleMin.value) / scaleRange : 0
  return CHART_PADDING.top + chartInnerHeight * (1 - Math.min(Math.max(ratio, 0), 1))
}

function makeChartPoints(series: DashboardDailyStat[], pickValue: (item: DashboardDailyStat) => number): ChartPoint[] {
  if (series.length === 0) {
    return []
  }
  return series.map((item, index) => {
    const x = series.length === 1
      ? CHART_PADDING.left
      : CHART_PADDING.left + (chartInnerWidth * index) / (series.length - 1)
    const value = pickValue(item)
    return {
      x,
      y: chartValueY(value),
      value,
      label: item.label,
    }
  })
}

function makeLinePath(points: ChartPoint[]): string {
  return points
    .map((point, index) => `${index === 0 ? 'M' : 'L'} ${point.x.toFixed(2)} ${point.y.toFixed(2)}`)
    .join(' ')
}
</script>

<template>
  <el-config-provider :locale="zhCn">
  <el-container class="app-shell">
    <el-aside class="sidebar" width="248px">
      <div class="brand-block">
        <div class="brand-mark">
          <el-icon><Box /></el-icon>
        </div>
        <div>
          <h1>医疗产品进销存</h1>
          <p>本地记录台账</p>
        </div>
      </div>

      <div class="nav-section-title">菜单</div>
      <el-menu :default-active="activePage" class="nav-menu" @select="activePage = String($event)">
        <el-menu-item v-for="item in navItems" :key="item.key" :index="item.key">
          <span class="nav-icon-wrap">
            <el-icon><component :is="item.icon" /></el-icon>
          </span>
          <span class="nav-label">{{ item.label }}</span>
          <span class="nav-count">{{ navCount(item.key) }}</span>
        </el-menu-item>
      </el-menu>

      <div class="sidebar-footer">
        <span>当前模块</span>
        <strong class="sidebar-current">{{ currentNavLabel }}</strong>
        <span>本地数据</span>
        <strong>{{ data.products.length + data.customers.length + data.orders.length + data.returns.length }}</strong>
        <small>产品 {{ data.products.length }} · 客户 {{ data.customers.length }} · 订单 {{ data.orders.length }} · 退货 {{ data.returns.length }}</small>
      </div>
    </el-aside>

    <el-container>
      <el-header class="topbar" height="72px">
        <div class="status-line">
          <strong>{{ statusMessage }}</strong>
          <span>上次保存：{{ data.savedAt }}</span>
        </div>
        <div class="top-actions">
          <el-button :icon="isDarkMode ? Sunny : Moon" @click="toggleTheme">
            {{ isDarkMode ? '明亮模式' : '暗黑模式' }}
          </el-button>
          <el-button :icon="DocumentAdd" @click="exportBackup">备份JSON</el-button>
          <el-button :icon="Upload" @click="openFile(backupFileInput)">恢复JSON</el-button>
          <input ref="backupFileInput" class="file-input" type="file" accept=".json" @change="importBackupJson" />
        </div>
      </el-header>

      <el-main class="workspace">
        <section v-if="activePage === 'dashboard'" class="page-stack dashboard-page">
          <div class="toolbar-band dashboard-toolbar">
            <div class="toolbar-left">
              <div class="dashboard-toolbar-title">
                <strong>经营看板</strong>
                <span>利润 / 退货 / 回款</span>
              </div>
              <el-date-picker v-model="dashboardFilter.startDate" type="date" value-format="YYYY-MM-DD" />
              <span class="date-separator">至</span>
              <el-date-picker v-model="dashboardFilter.endDate" type="date" value-format="YYYY-MM-DD" />
            </div>
            <div class="toolbar-right">
              <el-button @click="setDashboardRange('today')">今日</el-button>
              <el-button @click="setDashboardRange('week')">近7天</el-button>
              <el-button @click="setDashboardRange('month')">本月</el-button>
            </div>
          </div>

          <div class="metric-grid">
            <div class="metric-card accent-blue metric-primary">
              <span>累计净销售额</span>
              <strong>¥{{ formatMoney(netAmount) }}</strong>
              <small>订单金额 - 退货金额</small>
            </div>
            <div class="metric-card accent-green metric-primary">
              <span>累计净毛利</span>
              <strong>¥{{ formatMoney(netGrossProfit) }}</strong>
              <small>毛利率 {{ formatPercent(netGrossProfitRate) }}</small>
            </div>
            <div class="metric-card accent-blue">
              <span>今日净销售额</span>
              <strong>¥{{ formatMoney(todayNetAmount) }}</strong>
              <small>今日出库 - 今日退货</small>
            </div>
            <div class="metric-card accent-green">
              <span>今日净毛利</span>
              <strong>¥{{ formatMoney(todayNetGrossProfit) }}</strong>
              <small>毛利率 {{ formatPercent(todayNetGrossProfitRate) }}</small>
            </div>
            <div class="metric-card accent-red">
              <span>区间净销售额</span>
              <strong>¥{{ formatMoney(rangeNetAmount) }}</strong>
              <small>当前统计范围</small>
            </div>
            <div class="metric-card accent-green">
              <span>区间净毛利</span>
              <strong>¥{{ formatMoney(rangeGrossProfitAmount) }}</strong>
              <small>毛利率 {{ formatPercent(rangeGrossProfitRate) }}</small>
            </div>
          </div>

          <div class="dashboard-chart-grid">
            <div class="panel chart-panel">
              <div class="panel-head">
                <div>
                  <h2>利润趋势</h2>
                  <p class="panel-subtitle">按日期统计净销售额与净毛利</p>
                </div>
                <span>{{ dashboardFilter.startDate }} / {{ dashboardFilter.endDate }}</span>
              </div>
              <div class="chart-summary">
                <div>
                  <span>区间净销售</span>
                  <strong>¥{{ formatMoney(rangeNetAmount) }}</strong>
                </div>
                <div>
                  <span>区间净毛利</span>
                  <strong>¥{{ formatMoney(rangeGrossProfitAmount) }}</strong>
                </div>
                <div>
                  <span>毛利率</span>
                  <strong>{{ formatPercent(rangeGrossProfitRate) }}</strong>
                </div>
              </div>
              <div class="chart-legend">
                <span><i class="legend-dot outbound" />净销售额 <strong>{{ formatChartMoney(rangeNetAmount) }}</strong></span>
                <span><i class="legend-dot profit" />净毛利 <strong>{{ formatChartMoney(rangeGrossProfitAmount) }}</strong></span>
              </div>
              <div class="line-chart-wrap">
                <svg
                  class="line-chart"
                  :viewBox="`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`"
                  role="img"
                  aria-label="日期趋势折线图"
                >
                  <g class="chart-grid-lines">
                    <line
                      v-for="tick in chartTicks"
                      :key="tick.y"
                      :x1="CHART_PADDING.left"
                      :x2="CHART_WIDTH - CHART_PADDING.right"
                      :y1="tick.y"
                      :y2="tick.y"
                    />
                  </g>
                  <line
                    class="chart-zero-line"
                    :x1="CHART_PADDING.left"
                    :x2="CHART_WIDTH - CHART_PADDING.right"
                    :y1="chartValueY(0)"
                    :y2="chartValueY(0)"
                  />
                  <g class="chart-axis-labels">
                    <text
                      v-for="tick in chartTicks"
                      :key="tick.value"
                      :x="CHART_PADDING.left - 12"
                      :y="tick.y + 4"
                      text-anchor="end"
                    >
                      {{ formatChartMoney(tick.value) }}
                    </text>
                    <text
                      v-for="label in chartLabels"
                      :key="label.label"
                      :x="label.x"
                      :y="CHART_HEIGHT - 12"
                      text-anchor="middle"
                    >
                      {{ label.label }}
                    </text>
                  </g>
                  <path v-if="chartAreaPath" class="line-area outbound" :d="chartAreaPath" />
                  <path v-if="netSalesChartPath" class="line-path outbound" :d="netSalesChartPath" />
                  <path v-if="grossProfitChartPath" class="line-path profit" :d="grossProfitChartPath" />
                  <g class="chart-points outbound">
                    <circle
                      v-for="point in netSalesChartPoints"
                      :key="`net-sales-${point.label}`"
                      :cx="point.x"
                      :cy="point.y"
                      r="4"
                    >
                      <title>{{ point.label }} 净销售额 ¥{{ formatMoney(point.value) }}</title>
                    </circle>
                  </g>
                  <g class="chart-points profit">
                    <circle
                      v-for="point in grossProfitChartPoints"
                      :key="`gross-profit-${point.label}`"
                      :cx="point.x"
                      :cy="point.y"
                      r="4"
                    >
                      <title>{{ point.label }} 净毛利 ¥{{ formatMoney(point.value) }}</title>
                    </circle>
                  </g>
                </svg>
                <el-empty v-if="dailySeries.length === 0" description="暂无趋势数据" :image-size="96" />
              </div>
            </div>

            <div class="panel pie-panel">
              <div class="panel-head">
                <div>
                  <h2>金额占比</h2>
                  <p class="panel-subtitle">当前统计范围</p>
                </div>
              </div>
              <div class="pie-chart-wrap">
                <svg class="pie-chart" viewBox="0 0 180 180" role="img" aria-label="出库和退货金额占比饼图">
                  <circle class="pie-ring-bg" cx="90" cy="90" :r="PIE_RADIUS" />
                  <circle
                    v-for="slice in pieSlices"
                    :key="slice.label"
                    class="pie-slice"
                    :class="slice.className"
                    cx="90"
                    cy="90"
                    :r="PIE_RADIUS"
                    :stroke-dasharray="slice.dashArray"
                    :stroke-dashoffset="slice.dashOffset"
                  >
                    <title>{{ slice.label }} {{ Math.round(slice.percent * 100) }}%</title>
                  </circle>
                  <text x="90" y="84" text-anchor="middle">净额</text>
                  <text x="90" y="107" text-anchor="middle">¥{{ formatMoney(rangeNetAmount) }}</text>
                </svg>
                <el-empty v-if="!chartHasData" description="暂无占比数据" :image-size="84" />
              </div>
              <div class="pie-stats">
                <div v-for="slice in pieSlices" :key="slice.label" class="pie-stat">
                  <span><i class="legend-dot" :class="slice.className" />{{ slice.label }}</span>
                  <strong>{{ Math.round(slice.percent * 100) }}%</strong>
                  <small>¥{{ formatMoney(slice.value) }}</small>
                </div>
              </div>
            </div>
          </div>

          <div class="analysis-strip">
            <div class="analysis-item">
              <span>区间出库金额</span>
              <strong>¥{{ formatMoney(rangeOutboundAmount) }}</strong>
            </div>
            <div class="analysis-item">
              <span>区间出库数量</span>
              <strong>{{ formatMoney(rangeOutboundQuantity) }}</strong>
            </div>
            <div class="analysis-item">
              <span>退货冲减</span>
              <strong>¥{{ formatMoney(rangeReturnAmount) }}</strong>
              <small>占出库 {{ formatPercent(rangeReturnImpactRate) }}</small>
            </div>
            <div class="analysis-item">
              <span>区间退货数量</span>
              <strong>{{ formatMoney(rangeReturnQuantity) }}</strong>
            </div>
            <div class="analysis-item">
              <span>区间净销售</span>
              <strong>¥{{ formatMoney(rangeNetAmount) }}</strong>
            </div>
            <div class="analysis-item">
              <span>区间净毛利</span>
              <strong>¥{{ formatMoney(rangeGrossProfitAmount) }}</strong>
              <small>毛利率 {{ formatPercent(rangeGrossProfitRate) }}</small>
            </div>
            <div class="analysis-item">
              <span>发票回款率</span>
              <strong>{{ formatPercent(rangePaidRate) }}</strong>
              <small>未回 ¥{{ formatMoney(rangeUnpaidInvoiceAmount) }}</small>
            </div>
          </div>
        </section>

        <section v-if="activePage === 'products'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>产品列表</h2>
              <div class="panel-actions">
                <el-input v-model="productQuery" class="search-input" :prefix-icon="Search" clearable placeholder="名称、品牌、规格" />
                <el-tooltip content="新增产品" placement="top">
                  <el-button class="toolbar-icon-button" type="primary" :icon="Plus" circle @click="openProductCreate" />
                </el-tooltip>
                <el-tooltip content="导入Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Upload" circle @click="openImportDialog('products')" />
                </el-tooltip>
                <el-tooltip content="导出Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Download" circle @click="exportProductsExcel" />
                </el-tooltip>
                <input ref="productFileInput" class="file-input" type="file" accept=".xlsx,.xls" @change="importProductsExcel" />
              </div>
            </div>
            <div class="summary-strip">
              <span>全部 {{ data.products.length }}</span>
              <span>显示 {{ filteredProducts.length }}</span>
              <span>可直接下单 {{ data.products.length }}</span>
            </div>
            <el-table :data="pagedProducts" :height="tableHeight" stripe empty-text="暂无产品">
              <el-table-column label="产品名称" min-width="150" fixed>
                <template #default="{ row }">
                  <span class="table-link" @click="openOrdersForProduct(row)">{{ row.name }}</span>
                </template>
              </el-table-column>
              <el-table-column prop="brand" label="品牌" min-width="120" />
              <el-table-column prop="specification" label="规格/型号" min-width="140" />
              <el-table-column prop="unit" label="单位" width="90" />
              <el-table-column label="进货" width="110"><template #default="{ row }">¥{{ formatMoney(row.purchasePrice) }}</template></el-table-column>
              <el-table-column label="出库" width="110"><template #default="{ row }">¥{{ formatMoney(row.outboundPrice) }}</template></el-table-column>
              <el-table-column prop="description" label="描述" min-width="180" />
              <el-table-column label="操作" width="150" fixed="right">
                <template #default="{ row }">
                  <span class="table-actions">
                    <span class="table-action" @click="prepareOrder(row)">下单</span>
                    <span class="table-action" @click="editProduct(row)">编辑</span>
                    <span class="table-action danger" @click="deleteProduct(row)">删除</span>
                  </span>
                </template>
              </el-table-column>
            </el-table>
            <div class="pagination-bar">
              <el-pagination
                v-model:current-page="productPage"
                v-model:page-size="productPageSize"
                :total="filteredProducts.length"
                background
                layout="total, prev, pager, next"
              />
            </div>
          </div>
        </section>

        <section v-if="activePage === 'customers'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>客户列表</h2>
              <div class="panel-actions">
                <el-input v-model="customerQuery" class="search-input" :prefix-icon="Search" clearable placeholder="单位、联系人、电话" />
                <el-tooltip content="新增客户" placement="top">
                  <el-button class="toolbar-icon-button" type="primary" :icon="Plus" circle @click="openCustomerCreate" />
                </el-tooltip>
              </div>
            </div>
            <div class="summary-strip">
              <span>全部 {{ data.customers.length }}</span>
              <span>显示 {{ filteredCustomers.length }}</span>
              <span>订单 {{ data.orders.length }}</span>
            </div>
            <el-table :data="pagedCustomers" :height="tableHeight" stripe empty-text="暂无客户">
              <el-table-column prop="unit" label="客户单位" min-width="180" fixed />
              <el-table-column prop="contactName" label="联系人" min-width="120" />
              <el-table-column prop="phone" label="电话" min-width="130" />
              <el-table-column prop="address" label="地址" min-width="180" />
              <el-table-column label="订单数" width="100"><template #default="{ row }"><span class="table-link" @click="openOrdersForCustomer(row)">{{ customerOrderCount(row) }}</span></template></el-table-column>
              <el-table-column label="下单数量" width="120"><template #default="{ row }"><span class="table-link" @click="openOrdersForCustomer(row)">{{ formatMoney(customerOrderQuantity(row)) }}</span></template></el-table-column>
              <el-table-column label="退货数量" width="120"><template #default="{ row }"><span class="table-link" @click="openReturnsForCustomer(row)">{{ formatMoney(customerReturnQuantity(row)) }}</span></template></el-table-column>
              <el-table-column prop="remark" label="备注" min-width="160" />
              <el-table-column label="操作" width="155" fixed="right">
                <template #default="{ row }">
                  <span class="table-actions">
                    <span class="table-action" @click="openBatchOrderForCustomer(row)">下单</span>
                    <span class="table-action" @click="editCustomer(row)">编辑</span>
                    <span class="table-action danger" @click="deleteCustomer(row)">删除</span>
                  </span>
                </template>
              </el-table-column>
            </el-table>
            <div class="pagination-bar">
              <el-pagination
                v-model:current-page="customerPage"
                v-model:page-size="customerPageSize"
                :total="filteredCustomers.length"
                background
                layout="total, prev, pager, next"
              />
            </div>
          </div>
        </section>

        <section v-if="activePage === 'orders'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>出库单列表</h2>
              <div class="panel-actions">
                <el-input v-model="orderQuery" class="search-input" :prefix-icon="Search" clearable placeholder="订单编号、品名、货号、单位、订货人" />
                <el-date-picker v-model="orderDateRange" class="range-input" type="daterange" value-format="YYYY-MM-DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" />
                <el-tooltip content="新增出库单" placement="top">
                  <el-button class="toolbar-icon-button" type="primary" :icon="Plus" circle @click="openOrderCreate" />
                </el-tooltip>
                <el-tooltip content="生成发票" placement="top">
                  <el-button class="toolbar-icon-button" :icon="DocumentAdd" circle :disabled="selectedOrders.length === 0" @click="openInvoiceCreateFromSelectedOrders" />
                </el-tooltip>
                <el-tooltip content="打印出库单" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Printer" circle :disabled="selectedOrders.length === 0 || isPrinting" @click="printSelectedOrders" />
                </el-tooltip>
                <el-tooltip content="打印设置" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Setting" circle @click="printSettingsDialogVisible = true" />
                </el-tooltip>
                <el-tooltip content="导入Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Upload" circle @click="openImportDialog('orders')" />
                </el-tooltip>
                <el-tooltip content="导出Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Download" circle @click="exportOrdersExcel" />
                </el-tooltip>
                <input ref="orderFileInput" class="file-input" type="file" accept=".xlsx,.xls" @change="importOrdersExcel" />
              </div>
            </div>
            <div class="summary-strip">
              <span>全部 {{ data.orders.length }}</span>
              <span>可退 {{ returnableOrderCount }}</span>
              <span>当前出库 ¥{{ formatMoney(currentOutboundTotal) }}</span>
            </div>
            <el-table :data="pagedOrders" :height="tableHeight" row-key="id" stripe empty-text="暂无订单" @selection-change="handleOrderSelectionChange">
              <el-table-column type="selection" width="46" fixed />
              <el-table-column prop="orderNo" label="订单编号" min-width="260" fixed />
              <el-table-column prop="month" label="月份" width="90" />
              <el-table-column prop="orderTime" label="订购时间" width="120" />
              <el-table-column prop="customerUnit" label="订货单位" min-width="150" />
              <el-table-column prop="customerName" label="订货人" min-width="110" />
              <el-table-column prop="brand" label="品牌" min-width="110" />
              <el-table-column prop="itemNo" label="货号" min-width="160" />
              <el-table-column prop="productName" label="品名" min-width="150" />
              <el-table-column prop="unit" label="单位" width="80" />
              <el-table-column label="目录价" width="110"><template #default="{ row }">¥{{ formatMoney(row.catalogPrice) }}</template></el-table-column>
              <el-table-column label="数量" width="130"><template #default="{ row }">{{ formatMoney(row.quantity) }} {{ row.unit }}</template></el-table-column>
              <el-table-column label="开票总价" width="130"><template #default="{ row }">¥{{ formatMoney(row.invoiceTotal) }}</template></el-table-column>
              <el-table-column label="开票情况" width="110">
                <template #default="{ row }">
                  <el-tag :type="row.invoiceStatus === '全部开票' ? 'success' : row.invoiceStatus === '部分开票' ? 'warning' : 'info'" effect="light">
                    {{ row.invoiceStatus || '未开票' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="是否发货" width="100"><template #default="{ row }">{{ yesNo(row.isShipped) }}</template></el-table-column>
              <el-table-column label="返现" width="105"><template #default="{ row }">¥{{ formatMoney(row.cashback) }}</template></el-table-column>
              <el-table-column label="成本折扣" width="105"><template #default="{ row }">{{ formatMoney(row.costDiscount) }}</template></el-table-column>
              <el-table-column label="成本单价" width="115"><template #default="{ row }">¥{{ formatMoney(row.costUnitPrice) }}</template></el-table-column>
              <el-table-column label="成本总价" width="130"><template #default="{ row }">¥{{ formatMoney(row.costTotal) }}</template></el-table-column>
              <el-table-column label="售价折扣" width="105"><template #default="{ row }">{{ formatMoney(row.saleDiscount) }}</template></el-table-column>
              <el-table-column label="开票单价" width="120"><template #default="{ row }">¥{{ formatMoney(row.invoiceUnitPrice) }}</template></el-table-column>
              <el-table-column label="毛利" width="120"><template #default="{ row }">¥{{ formatMoney(row.grossProfit) }}</template></el-table-column>
              <el-table-column prop="remark" label="备注" min-width="150" />
              <el-table-column prop="invoiceNo" label="发票号" min-width="160" />
              <el-table-column label="是否回款" width="100"><template #default="{ row }">{{ yesNo(row.isPaid) }}</template></el-table-column>
              <el-table-column prop="paidTime" label="回款时间" width="120" />
              <el-table-column label="退货数量" width="120"><template #default="{ row }">{{ formatMoney(row.returnedQuantity) }} {{ row.unit }}</template></el-table-column>
              <el-table-column label="当前出库" width="120"><template #default="{ row }">{{ formatMoney(currentOutboundQuantity(row)) }}</template></el-table-column>
              <el-table-column label="操作" width="175" fixed="right">
                <template #default="{ row }">
                  <span class="table-actions">
                    <span class="table-action" @click="printOrder(row)">打印</span>
                    <span class="table-action" @click="prepareReturn(row)">退货</span>
                    <span class="table-action" @click="editOrder(row)">编辑</span>
                    <span class="table-action danger" @click="deleteOrder(row)">删除</span>
                  </span>
                </template>
              </el-table-column>
            </el-table>
            <div class="amount-total-bar">
              <strong>金额总计</strong>
              <span>数量 {{ formatMoney(filteredOrderAmountSummary.quantity) }}</span>
              <span>均开票单价 ¥{{ formatMoney(filteredOrderAmountSummary.averageInvoiceUnitPrice) }}</span>
              <span>均成本单价 ¥{{ formatMoney(filteredOrderAmountSummary.averageCostUnitPrice) }}</span>
              <span>开票总价 ¥{{ formatMoney(filteredOrderAmountSummary.invoiceTotal) }}</span>
              <span>成本总价 ¥{{ formatMoney(filteredOrderAmountSummary.costTotal) }}</span>
              <span>返现 ¥{{ formatMoney(filteredOrderAmountSummary.cashback) }}</span>
              <span>毛利 ¥{{ formatMoney(filteredOrderAmountSummary.grossProfit) }}</span>
            </div>
            <div class="pagination-bar">
              <el-pagination
                v-model:current-page="orderPage"
                v-model:page-size="orderPageSize"
                :total="filteredOrders.length"
                background
                layout="total, prev, pager, next"
              />
            </div>
          </div>
        </section>

        <section v-if="activePage === 'invoices'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>发票管理</h2>
              <div class="panel-actions">
                <el-input v-model="invoiceQuery" class="search-input" :prefix-icon="Search" clearable placeholder="发票号、出库单、客户、产品编号、备注" />
                <el-date-picker v-model="invoiceDateRange" class="range-input" type="daterange" value-format="YYYY-MM-DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" />
                <el-tooltip content="生成发票" placement="top">
                  <el-button class="toolbar-icon-button" type="primary" :icon="DocumentAdd" circle :disabled="selectedOrders.length === 0" @click="openInvoiceCreateFromSelectedOrders" />
                </el-tooltip>
                <el-tooltip content="导出Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Download" circle @click="exportInvoicesExcel" />
                </el-tooltip>
              </div>
            </div>
            <div class="summary-strip">
              <span>全部 {{ data.invoices.length }}</span>
              <span>显示 {{ filteredInvoices.length }}</span>
              <span>发票总额 ¥{{ formatMoney(invoiceTotalAmount) }}</span>
              <span>当前筛选 ¥{{ formatMoney(filteredInvoiceTotalAmount) }}</span>
            </div>
            <el-table :data="pagedInvoices" :height="tableHeight" row-key="id" stripe empty-text="暂无发票">
              <el-table-column label="发票号" min-width="220" fixed>
                <template #default="{ row }">
                  <span class="table-link" @click="openOrdersForInvoice(row)">{{ row.invoiceNo }}</span>
                </template>
              </el-table-column>
              <el-table-column prop="invoiceDate" label="开票日期" width="120" />
              <el-table-column label="开票总额" width="130"><template #default="{ row }">¥{{ formatMoney(row.totalAmount) }}</template></el-table-column>
              <el-table-column label="是否回款" width="100"><template #default="{ row }">{{ yesNo(row.isPaid) }}</template></el-table-column>
              <el-table-column prop="paidTime" label="回款时间" width="120" />
              <el-table-column label="关联出库单" min-width="320">
                <template #default="{ row }">
                  {{ formatInvoiceLineSummary(row.lines) }}
                </template>
              </el-table-column>
              <el-table-column prop="remark" label="备注" min-width="160" />
              <el-table-column label="操作" width="120" fixed="right">
                <template #default="{ row }">
                  <span class="table-actions">
                    <span class="table-action" @click="editInvoice(row)">编辑</span>
                    <span class="table-action danger" @click="deleteInvoice(row)">删除</span>
                  </span>
                </template>
              </el-table-column>
            </el-table>
            <div class="pagination-bar">
              <el-pagination
                v-model:current-page="invoicePage"
                v-model:page-size="invoicePageSize"
                :total="filteredInvoices.length"
                background
                layout="total, prev, pager, next"
              />
            </div>
          </div>
        </section>

        <section v-if="activePage === 'returns'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>退货记录</h2>
              <div class="panel-actions">
                <el-input v-model="returnQuery" class="search-input" :prefix-icon="Search" clearable placeholder="产品、货号、单位、订货人" />
                <el-date-picker v-model="returnDateRange" class="range-input" type="daterange" value-format="YYYY-MM-DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" />
                <el-tooltip content="新增退货" placement="top">
                  <el-button class="toolbar-icon-button" type="primary" :icon="Plus" circle @click="openReturnCreate" />
                </el-tooltip>
                <el-tooltip content="导入Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Upload" circle @click="openImportDialog('returns')" />
                </el-tooltip>
                <el-tooltip content="导出Excel" placement="top">
                  <el-button class="toolbar-icon-button" :icon="Download" circle @click="exportReturnsExcel" />
                </el-tooltip>
                <input ref="returnFileInput" class="file-input" type="file" accept=".xlsx,.xls" @change="importReturnsExcel" />
              </div>
            </div>
            <div class="summary-strip">
              <span>全部 {{ data.returns.length }}</span>
              <span>显示 {{ filteredReturns.length }}</span>
              <span>退货金额 ¥{{ formatMoney(totalReturnAmount) }}</span>
            </div>
            <el-table :data="pagedReturns" :height="tableHeight" stripe empty-text="暂无退货记录">
              <el-table-column prop="productName" label="产品" min-width="150" fixed />
              <el-table-column prop="itemNo" label="货号" min-width="110" />
              <el-table-column prop="customerUnit" label="订货单位" min-width="150" />
              <el-table-column prop="returnTime" label="退货日期" width="120" />
              <el-table-column label="数量" width="130"><template #default="{ row }">{{ formatMoney(row.quantity) }} {{ row.unit }}</template></el-table-column>
              <el-table-column label="销售单价" width="120"><template #default="{ row }">¥{{ formatMoney(row.invoiceUnitPrice) }}</template></el-table-column>
              <el-table-column label="成本价" width="115"><template #default="{ row }">¥{{ formatMoney(row.costUnitPrice) }}</template></el-table-column>
              <el-table-column label="退货金额" width="130"><template #default="{ row }">¥{{ formatMoney(row.invoiceTotal) }}</template></el-table-column>
              <el-table-column label="成本总计" width="130"><template #default="{ row }">¥{{ formatMoney(row.costTotal) }}</template></el-table-column>
              <el-table-column label="返现" width="105"><template #default="{ row }">¥{{ formatMoney(row.cashback) }}</template></el-table-column>
              <el-table-column label="毛利" width="120"><template #default="{ row }">¥{{ formatMoney(row.grossProfit) }}</template></el-table-column>
              <el-table-column label="操作" width="105" fixed="right">
                <template #default="{ row }">
                  <span class="table-actions">
                    <span class="table-action" @click="editReturn(row)">编辑</span>
                    <span class="table-action danger" @click="deleteReturn(row)">删除</span>
                  </span>
                </template>
              </el-table-column>
            </el-table>
            <div class="amount-total-bar">
              <strong>金额总计</strong>
              <span>数量 {{ formatMoney(filteredReturnAmountSummary.quantity) }}</span>
              <span>均销售单价 ¥{{ formatMoney(filteredReturnAmountSummary.averageInvoiceUnitPrice) }}</span>
              <span>均成本价 ¥{{ formatMoney(filteredReturnAmountSummary.averageCostUnitPrice) }}</span>
              <span>销售总价 ¥{{ formatMoney(filteredReturnAmountSummary.invoiceTotal) }}</span>
              <span>成本总计 ¥{{ formatMoney(filteredReturnAmountSummary.costTotal) }}</span>
              <span>返现 ¥{{ formatMoney(filteredReturnAmountSummary.cashback) }}</span>
              <span>毛利 ¥{{ formatMoney(filteredReturnAmountSummary.grossProfit) }}</span>
            </div>
            <div class="pagination-bar">
              <el-pagination
                v-model:current-page="returnPage"
                v-model:page-size="returnPageSize"
                :total="filteredReturns.length"
                background
                layout="total, prev, pager, next"
              />
            </div>
          </div>
        </section>

        <el-dialog
          v-model="importDialogVisible"
          :title="`${activeImportLabel}Excel导入`"
          width="min(560px, calc(100vw - 32px))"
          class="form-dialog import-dialog"
        >
          <div class="import-flow">
            <div class="import-flow-row">
              <span>导入文件</span>
              <el-button type="primary" :icon="Upload" @click="triggerImportUpload">上传Excel</el-button>
            </div>
            <div class="import-flow-row">
              <span>导入模板</span>
              <el-button :icon="Download" @click="exportImportTemplate()">下载模板</el-button>
            </div>
          </div>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="importDialogVisible = false">关闭</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="productDialogVisible"
          :title="editingProductId ? '编辑产品' : '新增产品'"
          width="min(880px, calc(100vw - 32px))"
          class="form-dialog"
          destroy-on-close
          @closed="resetProductForm"
        >
          <el-form label-position="top">
            <div class="form-grid">
              <el-form-item label="产品名称"><el-input v-model="productForm.name" /></el-form-item>
              <el-form-item label="品牌"><el-input v-model="productForm.brand" /></el-form-item>
              <el-form-item label="规格/型号"><el-input v-model="productForm.specification" /></el-form-item>
              <el-form-item label="单位"><el-input v-model="productForm.unit" /></el-form-item>
              <el-form-item label="进货金额"><el-input v-model="productForm.purchasePrice" /></el-form-item>
              <el-form-item label="出库金额"><el-input v-model="productForm.outboundPrice" /></el-form-item>
            </div>
            <el-form-item label="描述"><el-input v-model="productForm.description" type="textarea" :rows="4" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetProductForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveProduct">保存产品</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="customerDialogVisible"
          :title="editingCustomerId ? '编辑客户' : '新增客户'"
          width="min(760px, calc(100vw - 32px))"
          class="form-dialog"
          destroy-on-close
          @closed="resetCustomerForm"
        >
          <el-form label-position="top">
            <div class="form-grid">
              <el-form-item label="客户单位"><el-input v-model="customerForm.unit" /></el-form-item>
              <el-form-item label="联系人"><el-input v-model="customerForm.contactName" /></el-form-item>
              <el-form-item label="电话"><el-input v-model="customerForm.phone" /></el-form-item>
              <el-form-item label="地址"><el-input v-model="customerForm.address" /></el-form-item>
            </div>
            <el-form-item label="备注"><el-input v-model="customerForm.remark" type="textarea" :rows="3" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetCustomerForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveCustomer">保存客户</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="batchOrderDialogVisible"
          title="客户下单"
          width="min(1180px, calc(100vw - 32px))"
          class="form-dialog batch-order-dialog"
          destroy-on-close
          @closed="resetBatchOrderForm"
        >
          <el-form label-position="top">
            <div class="batch-customer-band">
              <div>
                <span>客户单位</span>
                <strong>{{ batchOrderCustomer?.unit || '-' }}</strong>
              </div>
              <div>
                <span>联系人</span>
                <strong>{{ batchOrderCustomer?.contactName || '-' }}</strong>
              </div>
              <div>
                <span>电话</span>
                <strong>{{ batchOrderCustomer?.phone || '-' }}</strong>
              </div>
            </div>
            <div class="form-grid">
              <el-form-item label="订购时间"><el-date-picker v-model="batchOrderTime" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="出库日期"><el-date-picker v-model="batchDeliveryDate" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="付款时间"><el-date-picker v-model="batchPaidTime" type="date" value-format="YYYY-MM-DD" /></el-form-item>
            </div>
            <div class="batch-line-head">
              <strong>产品明细</strong>
              <el-button type="primary" :icon="Plus" @click="addBatchOrderRow">添加产品</el-button>
            </div>
            <el-table :data="batchOrderRows" class="batch-line-table" max-height="420" stripe empty-text="暂无产品明细">
              <el-table-column label="产品" min-width="230" fixed>
                <template #default="{ row }">
                  <el-select v-model="row.productId" filterable clearable placeholder="选择产品" @change="applyProductToBatchRow(row)">
                    <el-option v-for="product in sortedProducts" :key="product.id" :label="`${product.name} | ${product.brand || '-'} | ${product.unit || '件'} | ¥${formatMoney(product.outboundPrice)}`" :value="product.id" />
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="货号" min-width="170">
                <template #default="{ row }"><el-input v-model="row.itemNo" /></template>
              </el-table-column>
              <el-table-column label="数量" width="105">
                <template #default="{ row }"><el-input v-model="row.quantity" @input="syncBatchOrderRowAmounts(row)" /></template>
              </el-table-column>
              <el-table-column label="销售单价" width="120">
                <template #default="{ row }"><el-input v-model="row.invoiceUnitPrice" @input="syncBatchOrderRowAmounts(row)" /></template>
              </el-table-column>
              <el-table-column label="成本价" width="120">
                <template #default="{ row }"><el-input v-model="row.costUnitPrice" @input="syncBatchOrderRowAmounts(row)" /></template>
              </el-table-column>
              <el-table-column label="销售总价" width="125">
                <template #default="{ row }"><el-input v-model="row.invoiceTotal" readonly /></template>
              </el-table-column>
              <el-table-column label="成本总计" width="125">
                <template #default="{ row }"><el-input v-model="row.costTotal" readonly /></template>
              </el-table-column>
              <el-table-column label="返现" width="110">
                <template #default="{ row }"><el-input v-model="row.cashback" @input="syncBatchOrderRowAmounts(row)" /></template>
              </el-table-column>
              <el-table-column label="毛利" width="120">
                <template #default="{ row }"><el-input v-model="row.grossProfit" readonly /></template>
              </el-table-column>
              <el-table-column label="备注" min-width="160">
                <template #default="{ row }"><el-input v-model="row.remark" /></template>
              </el-table-column>
              <el-table-column label="操作" width="76" fixed="right">
                <template #default="{ row }">
                  <span class="table-action danger" @click="removeBatchOrderRow(row.id)">删除</span>
                </template>
              </el-table-column>
            </el-table>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetBatchOrderForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveBatchOrders">保存订单</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="orderDialogVisible"
          :title="editingOrderId ? '编辑出库单' : '新增出库单'"
          width="min(980px, calc(100vw - 32px))"
          class="form-dialog"
          destroy-on-close
          @closed="resetOrderForm"
        >
          <el-form label-position="top">
            <el-form-item label="选择客户">
              <el-select v-model="orderForm.customerId" filterable clearable @change="applyCustomerToOrder">
                <el-option v-for="customer in sortedCustomers" :key="customer.id" :label="`${customer.unit} | ${customer.contactName || '-'}`" :value="customer.id" />
              </el-select>
            </el-form-item>
            <el-form-item label="选择产品">
              <el-select v-model="orderForm.productId" filterable clearable @change="applyProductToOrder">
                <el-option v-for="product in sortedProducts" :key="product.id" :label="`${product.name} | ${product.brand || '-'} | ${product.unit || '件'} | ¥${formatMoney(product.outboundPrice)}`" :value="product.id" />
              </el-select>
            </el-form-item>
            <div class="form-grid">
              <el-form-item label="订单编号"><el-input v-model="orderForm.orderNo" placeholder="留空自动生成" /></el-form-item>
              <el-form-item label="月份"><el-input v-model="orderForm.month" readonly /></el-form-item>
              <el-form-item label="品名"><el-input v-model="orderForm.productName" /></el-form-item>
              <el-form-item label="货号"><el-input v-model="orderForm.itemNo" /></el-form-item>
              <el-form-item label="订购时间"><el-date-picker v-model="orderForm.orderTime" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="订货单位"><el-input v-model="orderForm.customerUnit" /></el-form-item>
              <el-form-item label="订货人"><el-input v-model="orderForm.customerName" /></el-form-item>
              <el-form-item label="品牌"><el-input v-model="orderForm.brand" /></el-form-item>
              <el-form-item label="单位"><el-input v-model="orderForm.unit" /></el-form-item>
              <el-form-item label="目录价"><el-input v-model="orderForm.catalogPrice" /></el-form-item>
              <el-form-item label="数量"><el-input v-model="orderForm.quantity" /></el-form-item>
              <el-form-item label="开票单价"><el-input v-model="orderForm.invoiceUnitPrice" /></el-form-item>
              <el-form-item label="成本单价"><el-input v-model="orderForm.costUnitPrice" /></el-form-item>
              <el-form-item label="开票总价"><el-input v-model="orderForm.invoiceTotal" readonly /></el-form-item>
              <el-form-item label="成本总价"><el-input v-model="orderForm.costTotal" readonly /></el-form-item>
              <el-form-item label="返现"><el-input v-model="orderForm.cashback" /></el-form-item>
              <el-form-item label="成本折扣"><el-input v-model="orderForm.costDiscount" /></el-form-item>
              <el-form-item label="售价折扣"><el-input v-model="orderForm.saleDiscount" /></el-form-item>
              <el-form-item label="毛利"><el-input v-model="orderForm.grossProfit" readonly /></el-form-item>
              <el-form-item label="开票情况"><el-input v-model="orderForm.invoiceStatus" readonly /></el-form-item>
              <el-form-item label="发票号"><el-input v-model="orderForm.invoiceNo" disabled /></el-form-item>
              <el-form-item label="是否发货"><el-switch v-model="orderForm.isShipped" active-text="是" inactive-text="否" /></el-form-item>
              <el-form-item label="是否回款"><el-switch v-model="orderForm.isPaid" disabled active-text="是" inactive-text="否" /></el-form-item>
              <el-form-item label="回款时间"><el-date-picker v-model="orderForm.paidTime" type="date" value-format="YYYY-MM-DD" disabled /></el-form-item>
            </div>
            <el-form-item label="备注"><el-input v-model="orderForm.remark" type="textarea" :rows="3" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetOrderForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveOrder">保存出库单</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="invoiceDialogVisible"
          :title="editingInvoiceId ? '编辑发票' : '生成发票'"
          width="min(1080px, calc(100vw - 32px))"
          class="form-dialog"
          destroy-on-close
          @closed="resetInvoiceForm"
        >
          <el-form label-position="top">
            <div class="form-grid">
              <el-form-item label="发票号"><el-input v-model="invoiceForm.invoiceNo" /></el-form-item>
              <el-form-item label="开票日期"><el-date-picker v-model="invoiceForm.invoiceDate" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="是否回款"><el-switch v-model="invoiceForm.isPaid" active-text="是" inactive-text="否" /></el-form-item>
              <el-form-item label="回款时间"><el-date-picker v-model="invoiceForm.paidTime" type="date" value-format="YYYY-MM-DD" :disabled="!invoiceForm.isPaid" /></el-form-item>
            </div>
            <div class="batch-line-head">
              <strong>出库单明细</strong>
              <span>累计 ¥{{ formatMoney(invoiceFormTotalAmount) }}</span>
            </div>
            <el-table :data="invoiceForm.lines" class="batch-line-table" max-height="420" stripe empty-text="请从出库单列表多选后生成发票">
              <el-table-column prop="orderNo" label="订单编号" min-width="280" fixed />
              <el-table-column prop="customerUnit" label="订货单位" min-width="150" />
              <el-table-column prop="productName" label="品名" min-width="150" />
              <el-table-column prop="itemNo" label="货号" min-width="140" />
              <el-table-column label="出库单金额" width="130"><template #default="{ row }">¥{{ formatMoney(row.orderAmount) }}</template></el-table-column>
              <el-table-column label="本次发票金额" width="160">
                <template #default="{ row }"><el-input-number v-model="row.invoiceAmount" :min="0" :precision="2" :step="100" controls-position="right" /></template>
              </el-table-column>
              <el-table-column label="操作" width="76" fixed="right">
                <template #default="{ row }">
                  <span class="table-action danger" @click="removeInvoiceLine(row.id)">移除</span>
                </template>
              </el-table-column>
            </el-table>
            <el-form-item label="备注"><el-input v-model="invoiceForm.remark" type="textarea" :rows="3" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetInvoiceForm">取消</el-button>
              <el-button type="primary" :icon="DocumentAdd" @click="saveInvoice">保存发票</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="printSettingsDialogVisible"
          title="打印设置"
          width="min(680px, calc(100vw - 32px))"
          class="form-dialog"
        >
          <el-form label-position="top">
            <div class="form-grid print-settings-grid">
              <el-form-item label="出库单打印方式">
                <el-select v-model="printProfileKey" class="print-profile-select" size="default">
                  <el-option v-for="profile in PRINT_PROFILES" :key="profile.key" :label="profile.label" :value="profile.key" />
                </el-select>
              </el-form-item>
              <el-form-item label="单据编号前缀">
                <el-input v-model="printContentSettings.orderNoPrefix" placeholder="例如 No:" />
              </el-form-item>
              <el-form-item label="公司名称" class="form-item-wide">
                <el-input v-model="printContentSettings.companyName" placeholder="打印抬头公司名称" />
              </el-form-item>
              <el-form-item label="单据名称">
                <el-input v-model="printContentSettings.documentTitle" placeholder="例如 送（销）货单" />
              </el-form-item>
              <el-form-item label="出货仓">
                <el-input v-model="printContentSettings.warehouseName" placeholder="例如 普通舱" />
              </el-form-item>
              <el-form-item label="签字栏标题">
                <el-input v-model="printContentSettings.receiverSignatureLabel" placeholder="例如 收货人签名:" />
              </el-form-item>
              <el-form-item label="签字栏说明">
                <el-input v-model="printContentSettings.receiverSignatureHint" placeholder="例如 （“货物”“发票”已收到）" />
              </el-form-item>
            </div>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetPrintContentSettings">恢复默认</el-button>
              <el-button type="primary" @click="printSettingsDialogVisible = false">完成</el-button>
            </div>
          </template>
        </el-dialog>

        <el-dialog
          v-model="returnDialogVisible"
          :title="editingReturnId ? '编辑退货' : '新增退货'"
          width="min(980px, calc(100vw - 32px))"
          class="form-dialog"
          destroy-on-close
          @closed="resetReturnForm"
        >
          <el-form label-position="top">
            <el-form-item label="关联订单">
              <el-select v-model="returnForm.sourceOrderId" filterable clearable @change="fillReturnFromOrder">
                <el-option v-for="order in returnableOrders" :key="order.id" :label="`${order.productName} | ${order.customerUnit} | 可退 ${formatMoney(currentOutboundQuantity(order))}`" :value="order.id" />
              </el-select>
            </el-form-item>
            <div class="form-grid">
              <el-form-item label="产品名称"><el-input v-model="returnForm.productName" /></el-form-item>
              <el-form-item label="货号"><el-input v-model="returnForm.itemNo" /></el-form-item>
              <el-form-item label="退货日期"><el-date-picker v-model="returnForm.returnTime" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="订货单位"><el-input v-model="returnForm.customerUnit" /></el-form-item>
              <el-form-item label="订货人"><el-input v-model="returnForm.customerName" /></el-form-item>
              <el-form-item label="品牌"><el-input v-model="returnForm.brand" /></el-form-item>
              <el-form-item label="单位"><el-input v-model="returnForm.unit" /></el-form-item>
              <el-form-item label="数量"><el-input v-model="returnForm.quantity" /></el-form-item>
              <el-form-item label="销售单价"><el-input v-model="returnForm.invoiceUnitPrice" /></el-form-item>
              <el-form-item label="成本单价"><el-input v-model="returnForm.costUnitPrice" /></el-form-item>
              <el-form-item label="销售总价"><el-input v-model="returnForm.invoiceTotal" readonly /></el-form-item>
              <el-form-item label="成本总价"><el-input v-model="returnForm.costTotal" readonly /></el-form-item>
              <el-form-item label="返现"><el-input v-model="returnForm.cashback" /></el-form-item>
              <el-form-item label="毛利"><el-input v-model="returnForm.grossProfit" readonly /></el-form-item>
            </div>
            <el-form-item label="备注"><el-input v-model="returnForm.remark" type="textarea" :rows="3" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetReturnForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveReturn">保存退货</el-button>
            </div>
          </template>
        </el-dialog>
      </el-main>
    </el-container>
  </el-container>
  </el-config-provider>
</template>

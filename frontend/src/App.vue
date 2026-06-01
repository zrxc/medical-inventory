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
  Sunny,
  Tickets,
  Upload,
  User,
} from '@element-plus/icons-vue'
import type { AppData, Customer, CustomerForm, DailyStat, Order, OrderForm, Product, ProductForm, ReturnForm, ReturnRecord } from './types'
import {
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
  nowString,
  parseAmount,
  proratedAmount,
  todayString,
} from './inventory'

type ImportType = 'products' | 'orders' | 'returns'
type AmountSummaryRow = Pick<Order, 'quantity' | 'invoiceTotal' | 'cashback' | 'costTotal' | 'grossProfit'>

interface AmountSummary {
  quantity: number
  invoiceTotal: number
  cashback: number
  costTotal: number
  grossProfit: number
  averageInvoiceUnitPrice: number
  averageCostUnitPrice: number
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

const STORAGE_KEY = 'medical-inventory-vue-data'
const THEME_KEY = 'medical-inventory-theme'
const AUTO_BACKUP_STORAGE_KEY = 'medical-inventory-auto-backup'
const AUTO_BACKUP_FILENAME = 'inventory-auto-backup.json'
const AUTO_BACKUP_INTERVAL = 10 * 60 * 1000

const activePage = ref('dashboard')
const themeMode = ref<'light' | 'dark'>(localStorage.getItem(THEME_KEY) === 'dark' ? 'dark' : 'light')
const statusMessage = ref('就绪')
const productQuery = ref('')
const customerQuery = ref('')
const orderQuery = ref('')
const returnQuery = ref('')
const orderDateRange = ref<[string, string] | []>([])
const returnDateRange = ref<[string, string] | []>([])
const productPage = ref(1)
const productPageSize = ref(8)
const customerPage = ref(1)
const customerPageSize = ref(8)
const orderPage = ref(1)
const orderPageSize = ref(8)
const returnPage = ref(1)
const returnPageSize = ref(8)
const viewportHeight = ref(window.innerHeight)
const tableHeight = ref(520)
let tableHeightFrame = 0
let autoBackupTimer: ReturnType<typeof setInterval> | null = null
const editingProductId = ref<string | null>(null)
const editingCustomerId = ref<string | null>(null)
const editingOrderId = ref<string | null>(null)
const editingReturnId = ref<string | null>(null)
const productDialogVisible = ref(false)
const customerDialogVisible = ref(false)
const orderDialogVisible = ref(false)
const returnDialogVisible = ref(false)
const batchOrderDialogVisible = ref(false)
const productFileInput = ref<HTMLInputElement | null>(null)
const orderFileInput = ref<HTMLInputElement | null>(null)
const returnFileInput = ref<HTMLInputElement | null>(null)
const backupFileInput = ref<HTMLInputElement | null>(null)
const selectedOrders = ref<Order[]>([])
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

const isDarkMode = computed(() => themeMode.value === 'dark')
const autoPageSize = computed(() => Math.max(5, Math.floor((tableHeight.value - 54) / 46)))

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

watch(orderDateRange, () => {
  orderPage.value = 1
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
    returnPageSize.value = pageSize
  },
  { immediate: true },
)

watch([activePage, productQuery, customerQuery, orderQuery, returnQuery, orderDateRange, returnDateRange], () => {
  scheduleTableHeightUpdate()
})

const navItems = [
  { key: 'dashboard', label: '看板', icon: DataAnalysis },
  { key: 'products', label: '产品管理', icon: Goods },
  { key: 'customers', label: '客户管理', icon: User },
  { key: 'orders', label: '下单管理', icon: Tickets },
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
      order.productName,
      order.itemNo,
      order.customerUnit,
      order.customerName,
      order.remark,
    ])
    return queryMatched && dateInRange(order.deliveryDate, orderDateRange.value)
  }),
)

const pagedOrders = computed(() => paginate(filteredOrders.value, orderPage.value, orderPageSize.value))
const filteredOrderAmountSummary = computed(() => summarizeAmounts(filteredOrders.value))

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
const currentOutboundTotal = computed(() => data.orders.reduce((sum, order) => sum + currentOutboundAmount(order), 0))
const returnableOrderCount = computed(() => data.orders.filter((order) => currentOutboundQuantity(order) > 0).length)
const todayOutboundAmount = computed(() =>
  data.orders
    .filter((order) => normalizeDate(order.deliveryDate) === todayString())
    .reduce((sum, order) => sum + currentOutboundAmount(order), 0),
)
const todayReturnAmount = computed(() =>
  data.returns
    .filter((record) => normalizeDate(record.returnTime) === todayString())
    .reduce((sum, record) => sum + record.invoiceTotal, 0),
)

const dailySeries = computed<DailyStat[]>(() => {
  const start = dateValue(dashboardFilter.startDate)
  const end = dateValue(dashboardFilter.endDate)
  if (!start || !end || start > end) {
    return []
  }

  const result: DailyStat[] = []
  const cursor = new Date(start)
  const last = new Date(end)
  while (cursor <= last && result.length < 45) {
    const label = formatLocalDate(cursor)
    result.push({
      label,
      outboundAmount: data.orders
        .filter((order) => normalizeDate(order.deliveryDate) === label)
        .reduce((sum, order) => sum + currentOutboundAmount(order), 0),
      returnAmount: data.returns
        .filter((record) => normalizeDate(record.returnTime) === label)
        .reduce((sum, record) => sum + record.invoiceTotal, 0),
    })
    cursor.setDate(cursor.getDate() + 1)
  }
  return result
})

const maxDailyAmount = computed(() =>
  Math.max(1, ...dailySeries.value.flatMap((item) => [item.outboundAmount, item.returnAmount])),
)

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
    loaded.orders = loaded.orders.map((order) => ({
      ...order,
      customerId: order.customerId ?? findCustomerIdByText(loaded.customers, order.customerUnit, order.customerName),
    }))
    return loaded
  } catch {
    return defaultData()
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
    productId: null,
    customerId: null,
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
  if (key === 'returns') {
    return String(data.returns.length)
  }
  return String(data.products.length + data.customers.length + data.orders.length + data.returns.length)
}

function identity(value: string): string {
  return value.trim().toLowerCase()
}

function findCustomerIdByText(customers: Customer[], unit: string, contactName: string): string | null {
  const customer = customers.find(
    (item) => identity(item.unit) === identity(unit) && identity(item.contactName) === identity(contactName),
  )
  return customer?.id ?? null
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
        productId: row.productId,
        customerId: customer.id,
        productName,
        itemNo: row.itemNo.trim() || generateOrderItemNo(product, index),
        orderTime,
        deliveryDate,
        customerUnit: customer.unit,
        customerName: customer.contactName,
        brand: row.brand.trim() || product?.brand || '',
        unit: row.unit.trim() || product?.unit || '件',
        catalogPrice: invoiceUnitPrice,
        quantity,
        invoiceTotal,
        cashback,
        costDiscount: 0,
        costUnitPrice,
        costTotal,
        saleDiscount: 0,
        invoiceUnitPrice,
        grossProfit: invoiceTotal - costTotal - cashback,
        remark: row.remark.trim(),
        paidTime,
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

function generateOrderItemNo(product?: Product, sequenceOffset = 0): string {
  const prefix = product?.brand?.trim()
    ? product.brand.trim().slice(0, 2).toUpperCase()
    : 'ITEM'
  const dateText = todayString().replaceAll('-', '')
  const sequence = String(data.orders.length + sequenceOffset + 1).padStart(3, '0')
  return `${prefix}-${dateText}-${sequence}`
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

  return {
    id,
    productId: orderForm.productId,
    customerId: orderForm.customerId,
    productName: orderForm.productName.trim(),
    itemNo: orderForm.itemNo.trim() || generateOrderItemNo(data.products.find((product) => product.id === orderForm.productId) ?? undefined),
    orderTime,
    deliveryDate,
    customerUnit: orderForm.customerUnit.trim(),
    customerName: orderForm.customerName.trim(),
    brand: orderForm.brand.trim(),
    unit: orderForm.unit.trim() || '件',
    catalogPrice: parseNonNegative('目录价', orderForm.catalogPrice),
    quantity,
    invoiceTotal: parseNonNegative('销售总价', orderForm.invoiceTotal),
    cashback: parseNonNegative('返现', orderForm.cashback),
    costDiscount: parseNonNegative('成本折扣', orderForm.costDiscount),
    costUnitPrice: parseNonNegative('成本单价', orderForm.costUnitPrice),
    costTotal: parseNonNegative('成本总价', orderForm.costTotal),
    saleDiscount: parseNonNegative('售价折扣', orderForm.saleDiscount),
    invoiceUnitPrice: parseNonNegative('销售单价', orderForm.invoiceUnitPrice),
    grossProfit: parseAmount(orderForm.grossProfit),
    remark: orderForm.remark.trim(),
    paidTime: normalizeDate(orderForm.paidTime) || todayString(),
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
      editingOrderId.value = null
      Object.assign(orderForm, blankOrderForm())
      orderDialogVisible.value = false
      markSaved('订单已更新')
      return
    }

    data.orders.push(buildOrder(createId(), 0, nowString()))
    Object.assign(orderForm, blankOrderForm())
    orderDialogVisible.value = false
    markSaved('订单已新增')
  })
}

function openOrderCreate(): void {
  editingOrderId.value = null
  Object.assign(orderForm, blankOrderForm())
  orderDialogVisible.value = true
}

function editOrder(order: Order): void {
  Object.assign(orderForm, {
    productId: order.productId,
    customerId: order.customerId ?? findCustomerIdByText(data.customers, order.customerUnit, order.customerName),
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
    cashback: formatMoney(order.cashback),
    costDiscount: formatMoney(order.costDiscount),
    costUnitPrice: formatMoney(order.costUnitPrice),
    costTotal: formatMoney(order.costTotal),
    saleDiscount: formatMoney(order.saleDiscount),
    invoiceUnitPrice: formatMoney(order.invoiceUnitPrice),
    grossProfit: formatMoney(order.grossProfit),
    remark: order.remark,
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
  if (!(await confirmDanger(`确认删除订单“${order.productName} / ${order.customerUnit}”？`, '删除订单'))) {
    return
  }
  data.orders = data.orders.filter((item) => item.id !== order.id)
  markSaved('订单已删除')
}

function handleOrderSelectionChange(selection: Order[]): void {
  selectedOrders.value = selection
}

function printOrder(order: Order): void {
  printOrders([order])
}

function printSelectedOrders(): void {
  if (!selectedOrders.value.length) {
    ElMessage.warning('请先选择要打印的订单')
    return
  }
  printOrders(selectedOrders.value)
}

function printOrders(orders: Order[]): void {
  const frame = document.createElement('iframe')
  frame.title = '出库单打印'
  frame.style.position = 'fixed'
  frame.style.right = '0'
  frame.style.bottom = '0'
  frame.style.width = '0'
  frame.style.height = '0'
  frame.style.border = '0'
  frame.style.opacity = '0'
  frame.style.pointerEvents = 'none'
  document.body.appendChild(frame)

  const printDocument = frame.contentDocument ?? frame.contentWindow?.document
  const printWindow = frame.contentWindow
  if (!printDocument || !printWindow) {
    frame.remove()
    ElMessage.error('打印组件初始化失败，请重试')
    return
  }

  const cleanup = () => {
    setTimeout(() => {
      frame.remove()
    }, 300)
  }

  printDocument.open()
  printDocument.write(buildDeliveryPrintHtml(orders))
  printDocument.close()

  printWindow.addEventListener('afterprint', cleanup, { once: true })
  setTimeout(() => {
    printWindow.focus()
    printWindow.print()
    setTimeout(cleanup, 5000)
  }, 300)
  statusMessage.value = `已发送 ${orders.length} 张出库单到打印`
}

function buildDeliveryPrintHtml(orders: Order[]): string {
  const pages = orders.map((order, index) => buildDeliveryPrintPage(order, index, orders.length)).join('')
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>出库单打印</title>
  <style>
    @page { size: A4 landscape; margin: 6mm; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      color: #000;
      background: #fff;
      font-family: "SimSun", "宋体", serif;
    }
    .sheet {
      width: 268mm;
      max-width: 100%;
      min-height: 186mm;
      margin: 0 auto;
      page-break-after: always;
      padding: 0;
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
      font-size: 18pt;
      line-height: 1.38;
      font-weight: 700;
      letter-spacing: 0;
    }
    .delivery-title h2 {
      margin: 1mm 0 0;
      font-size: 15.5pt;
      line-height: 1.35;
      font-weight: 700;
      letter-spacing: 0;
    }
    .order-no {
      position: absolute;
      right: 1mm;
      bottom: 3mm;
      font-size: 13.5pt;
      font-weight: 700;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      table-layout: fixed;
      font-size: 13pt;
    }
    td, th {
      border: 1px solid #000;
      height: 12.5mm;
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
      font-size: 12.8pt;
      font-weight: 400;
    }
    .text-left { text-align: left; }
    .money { text-align: right; padding-right: 3mm; font-family: "Courier New", monospace; }
    .item-row td { height: 17mm; }
    .blank-row td { height: 12.5mm; }
    .footer td { height: 12.5mm; }
    .signature {
      height: 20mm;
      text-align: center;
      line-height: 1.8;
      font-size: 13pt;
    }
  </style>
</head>
<body>${pages}</body>
</html>`
}

function buildDeliveryPrintPage(order: Order, index: number, total: number): string {
  const product = data.products.find((item) => item.id === order.productId)
  const itemName = [order.productName, product?.specification].filter(Boolean).join(' ')
  const amount = order.invoiceTotal
  const row = `
    <tr class="item-row">
      <td>${escapeHtml(order.brand || '-')}</td>
      <td>${escapeHtml(order.itemNo || '-')}</td>
      <td>${escapeHtml(itemName || '-')}</td>
      <td>${escapeHtml(order.unit || '-')}</td>
      <td>${formatMoney(order.quantity)}</td>
      <td></td>
      <td>普通舱</td>
      <td></td>
      <td class="money">${formatMoney(order.invoiceUnitPrice)}</td>
      <td class="money">${formatMoney(amount)}</td>
    </tr>`
  const blanks = Array.from({ length: 5 }, () => `
    <tr class="blank-row">
      <td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td>
    </tr>`).join('')

  return `
  <section class="sheet">
    <div class="delivery-title">
      <h1>武汉维优诺生物科技有限公司</h1>
      <h2>送（销）货单</h2>
      <div class="order-no">No:${escapeHtml(buildPrintOrderNo(order, index, total))}</div>
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
        <td colspan="2">${escapeHtml(order.customerUnit || '')}</td>
        <td colspan="2">开单日期</td>
        <td colspan="5">${escapeHtml(formatPrintDate(order.deliveryDate))}</td>
      </tr>
      <tr>
        <td class="label">订货人</td>
        <td colspan="2">${escapeHtml(order.customerName || '')}</td>
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
      ${row}
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
          <div>收货人签名:</div>
          <div>（“货物”“发票”已收到）</div>
        </td>
        <td colspan="7"></td>
      </tr>
    </table>
  </section>`
}

function buildPrintOrderNo(order: Order, index: number, total: number): string {
  const dateText = (normalizeDate(order.deliveryDate) || todayString()).replaceAll('-', '')
  const suffix = total > 1 ? index + 1 : 1
  return `000-${dateText}${suffix}`
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
    return '下单'
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
    for (const row of rows) {
      const productName = row['产品名称']?.trim()
      if (!productName) {
        continue
      }
      const product = data.products.find(
        (item) => identity(item.name) === identity(productName) && identity(item.brand) === identity(row['品牌'] ?? ''),
      )
      data.orders.push({
        id: createId(),
        productId: product?.id ?? null,
        customerId: findCustomerIdByText(data.customers, row['订货单位'] ?? '', row['订货人'] ?? ''),
        productName,
        itemNo: row['货号'] ?? '',
        orderTime: normalizeDate(row['订购时间'] ?? '') || todayString(),
        deliveryDate: normalizeDate(row['出库日期'] ?? '') || todayString(),
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
        returnedQuantity: parseNonNegative('已退数量', row['已退数量'] ?? '0'),
        createdAt: row['创建时间'] || nowString(),
      })
      imported += 1
    }
    markSaved(`订单Excel已导入 ${imported} 条`)
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
  return RETURN_HEADERS
}

async function exportProductsExcel(): Promise<void> {
  await exportSingleSheetExcel('产品', PRODUCT_HEADERS, productExportRows(), '产品.xlsx', '产品Excel已导出')
}

async function exportOrdersExcel(): Promise<void> {
  await exportSingleSheetExcel('订单', ORDER_HEADERS, orderExportRows(), '订单.xlsx', '订单Excel已导出')
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
    order.productName,
    order.itemNo,
    order.orderTime,
    order.deliveryDate,
    order.customerUnit,
    order.customerName,
    order.brand,
    order.unit,
    formatMoney(order.catalogPrice),
    formatMoney(order.quantity),
    formatMoney(order.invoiceTotal),
    formatMoney(order.cashback),
    formatMoney(order.costDiscount),
    formatMoney(order.costUnitPrice),
    formatMoney(order.costTotal),
    formatMoney(order.saleDiscount),
    formatMoney(order.invoiceUnitPrice),
    formatMoney(order.grossProfit),
    order.remark,
    formatMoney(order.returnedQuantity),
    order.createdAt,
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
    '订单',
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

function barWidth(value: number): string {
  if (value <= 0) {
    return '0%'
  }
  return `${Math.max(4, (value / maxDailyAmount.value) * 100)}%`
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
        <section v-if="activePage === 'dashboard'" class="page-stack">
          <div class="toolbar-band">
            <div class="toolbar-left">
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
            <div class="metric-card">
              <span>产品数</span>
              <strong>{{ data.products.length }}</strong>
              <small>已登记产品</small>
            </div>
            <div class="metric-card">
              <span>订单数</span>
              <strong>{{ data.orders.length }}</strong>
              <small>累计订单</small>
            </div>
            <!-- <div class="metric-card">
              <span>客户数</span>
              <strong>{{ data.customers.length }}</strong>
              <small>已登记客户</small>
            </div> -->
            <div class="metric-card">
              <span>退货记录</span>
              <strong>{{ data.returns.length }}</strong>
              <small>累计退货</small>
            </div>
            <div class="metric-card accent-blue">
              <span>今日出库</span>
              <strong>¥{{ formatMoney(todayOutboundAmount) }}</strong>
              <small>按出库日期</small>
            </div>
            <div class="metric-card accent-red">
              <span>今日退货</span>
              <strong>¥{{ formatMoney(todayReturnAmount) }}</strong>
              <small>按退货日期</small>
            </div>
            <div class="metric-card accent-green">
              <span>累计金额</span>
              <strong>¥{{ formatMoney(netAmount) }}</strong>
              <small>订单减退货</small>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <h2>日期趋势</h2>
              <span>{{ dashboardFilter.startDate }} / {{ dashboardFilter.endDate }}</span>
            </div>
            <div class="trend-list">
              <div v-for="item in dailySeries" :key="item.label" class="trend-row">
                <span class="trend-date">{{ item.label }}</span>
                <div class="trend-bars">
                  <div class="trend-track">
                    <div class="trend-fill outbound" :style="{ width: barWidth(item.outboundAmount) }" />
                  </div>
                  <div class="trend-track">
                    <div class="trend-fill returns" :style="{ width: barWidth(item.returnAmount) }" />
                  </div>
                </div>
                <span class="trend-value">¥{{ formatMoney(item.outboundAmount) }} / ¥{{ formatMoney(item.returnAmount) }}</span>
              </div>
              <el-empty v-if="dailySeries.length === 0" description="暂无趋势数据" :image-size="96" />
            </div>
          </div>
        </section>

        <section v-if="activePage === 'products'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>产品列表</h2>
              <div class="panel-actions">
                <el-input v-model="productQuery" class="search-input" :prefix-icon="Search" clearable placeholder="名称、品牌、规格" />
                <el-button type="primary" :icon="Plus" @click="openProductCreate">新增产品</el-button>
                <el-button :icon="Upload" @click="openImportDialog('products')">导入Excel</el-button>
                <el-button :icon="Download" @click="exportProductsExcel">导出Excel</el-button>
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
                <el-button type="primary" :icon="Plus" @click="openCustomerCreate">新增客户</el-button>
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
              <h2>订单列表</h2>
              <div class="panel-actions">
                <el-input v-model="orderQuery" class="search-input" :prefix-icon="Search" clearable placeholder="产品、货号、单位、订货人" />
                <el-date-picker v-model="orderDateRange" class="range-input" type="daterange" value-format="YYYY-MM-DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" />
                <el-button type="primary" :icon="Plus" @click="openOrderCreate">新增订单</el-button>
                <el-button :icon="Printer" :disabled="selectedOrders.length === 0" @click="printSelectedOrders">打印出库单</el-button>
                <el-button :icon="Upload" @click="openImportDialog('orders')">导入Excel</el-button>
                <el-button :icon="Download" @click="exportOrdersExcel">导出Excel</el-button>
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
              <el-table-column prop="productName" label="产品" min-width="150" fixed />
              <el-table-column prop="itemNo" label="货号" width="200" />
              <el-table-column prop="customerUnit" label="订货单位" min-width="150" />
              <el-table-column prop="customerName" label="订货人" min-width="110" />
              <el-table-column prop="deliveryDate" label="出库日期" width="120" />
              <el-table-column label="数量" width="130"><template #default="{ row }">{{ formatMoney(row.quantity) }} {{ row.unit }}</template></el-table-column>
              <el-table-column label="销售单价" width="120"><template #default="{ row }">¥{{ formatMoney(row.invoiceUnitPrice) }}</template></el-table-column>
              <el-table-column label="成本价" width="115"><template #default="{ row }">¥{{ formatMoney(row.costUnitPrice) }}</template></el-table-column>
              <el-table-column label="退货数量" width="120"><template #default="{ row }">{{ formatMoney(row.returnedQuantity) }} {{ row.unit }}</template></el-table-column>
              <el-table-column label="状态" width="110">
                <template #default="{ row }">
                  <el-tag :type="currentOutboundQuantity(row) > 0 ? 'success' : 'info'" effect="light">
                    {{ currentOutboundQuantity(row) > 0 ? '可退' : '已退完' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="当前出库" width="120"><template #default="{ row }">{{ formatMoney(currentOutboundQuantity(row)) }}</template></el-table-column>
              <el-table-column label="销售总价" width="130"><template #default="{ row }">¥{{ formatMoney(row.invoiceTotal) }}</template></el-table-column>
              <el-table-column label="成本总计" width="130"><template #default="{ row }">¥{{ formatMoney(row.costTotal) }}</template></el-table-column>
              <el-table-column label="返现" width="105"><template #default="{ row }">¥{{ formatMoney(row.cashback) }}</template></el-table-column>
              <el-table-column label="毛利" width="120"><template #default="{ row }">¥{{ formatMoney(row.grossProfit) }}</template></el-table-column>
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
              <span>均销售单价 ¥{{ formatMoney(filteredOrderAmountSummary.averageInvoiceUnitPrice) }}</span>
              <span>均成本价 ¥{{ formatMoney(filteredOrderAmountSummary.averageCostUnitPrice) }}</span>
              <span>销售总价 ¥{{ formatMoney(filteredOrderAmountSummary.invoiceTotal) }}</span>
              <span>成本总计 ¥{{ formatMoney(filteredOrderAmountSummary.costTotal) }}</span>
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

        <section v-if="activePage === 'returns'" class="page-stack">
          <div class="panel table-panel">
            <div class="panel-head">
              <h2>退货记录</h2>
              <div class="panel-actions">
                <el-input v-model="returnQuery" class="search-input" :prefix-icon="Search" clearable placeholder="产品、货号、单位、订货人" />
                <el-date-picker v-model="returnDateRange" class="range-input" type="daterange" value-format="YYYY-MM-DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" />
                <el-button type="primary" :icon="Plus" @click="openReturnCreate">新增退货</el-button>
                <el-button :icon="Upload" @click="openImportDialog('returns')">导入Excel</el-button>
                <el-button :icon="Download" @click="exportReturnsExcel">导出Excel</el-button>
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
          :title="editingOrderId ? '编辑订单' : '新增订单'"
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
              <el-form-item label="产品名称"><el-input v-model="orderForm.productName" /></el-form-item>
              <el-form-item label="货号"><el-input v-model="orderForm.itemNo" /></el-form-item>
              <el-form-item label="订购时间"><el-date-picker v-model="orderForm.orderTime" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="出库日期"><el-date-picker v-model="orderForm.deliveryDate" type="date" value-format="YYYY-MM-DD" /></el-form-item>
              <el-form-item label="订货单位"><el-input v-model="orderForm.customerUnit" /></el-form-item>
              <el-form-item label="订货人"><el-input v-model="orderForm.customerName" /></el-form-item>
              <el-form-item label="品牌"><el-input v-model="orderForm.brand" /></el-form-item>
              <el-form-item label="单位"><el-input v-model="orderForm.unit" /></el-form-item>
              <el-form-item label="目录价"><el-input v-model="orderForm.catalogPrice" /></el-form-item>
              <el-form-item label="数量"><el-input v-model="orderForm.quantity" /></el-form-item>
              <el-form-item label="销售单价"><el-input v-model="orderForm.invoiceUnitPrice" /></el-form-item>
              <el-form-item label="成本单价"><el-input v-model="orderForm.costUnitPrice" /></el-form-item>
              <el-form-item label="销售总价"><el-input v-model="orderForm.invoiceTotal" readonly /></el-form-item>
              <el-form-item label="成本总价"><el-input v-model="orderForm.costTotal" readonly /></el-form-item>
              <el-form-item label="返现"><el-input v-model="orderForm.cashback" /></el-form-item>
              <el-form-item label="毛利"><el-input v-model="orderForm.grossProfit" readonly /></el-form-item>
            </div>
            <el-form-item label="备注"><el-input v-model="orderForm.remark" type="textarea" :rows="3" /></el-form-item>
          </el-form>
          <template #footer>
            <div class="dialog-actions">
              <el-button @click="resetOrderForm">取消</el-button>
              <el-button type="primary" :icon="Plus" @click="saveOrder">保存订单</el-button>
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

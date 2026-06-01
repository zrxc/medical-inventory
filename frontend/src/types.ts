export interface AppData {
  version: number
  savedAt: string
  products: Product[]
  customers: Customer[]
  orders: Order[]
  returns: ReturnRecord[]
}

export interface Customer {
  id: string
  unit: string
  contactName: string
  phone: string
  address: string
  remark: string
  createdAt: string
}

export interface Product {
  id: string
  name: string
  purchasePrice: number
  outboundPrice: number
  specification: string
  brand: string
  unit: string
  description: string
  createdAt: string
}

export interface Order {
  id: string
  productId: string | null
  customerId: string | null
  productName: string
  itemNo: string
  orderTime: string
  deliveryDate: string
  customerUnit: string
  customerName: string
  brand: string
  unit: string
  catalogPrice: number
  quantity: number
  invoiceTotal: number
  cashback: number
  costDiscount: number
  costUnitPrice: number
  costTotal: number
  saleDiscount: number
  invoiceUnitPrice: number
  grossProfit: number
  remark: string
  paidTime: string
  returnedQuantity: number
  createdAt: string
}

export interface ReturnRecord {
  id: string
  sourceOrderId: string
  productId: string | null
  productName: string
  itemNo: string
  returnTime: string
  customerUnit: string
  customerName: string
  brand: string
  unit: string
  catalogPrice: number
  quantity: number
  invoiceTotal: number
  cashback: number
  costDiscount: number
  costUnitPrice: number
  costTotal: number
  saleDiscount: number
  invoiceUnitPrice: number
  grossProfit: number
  remark: string
  paidTime: string
  createdAt: string
}

export interface ProductForm {
  name: string
  purchasePrice: string
  outboundPrice: string
  specification: string
  brand: string
  unit: string
  description: string
}

export interface OrderForm {
  productId: string | null
  customerId: string | null
  productName: string
  itemNo: string
  orderTime: string
  deliveryDate: string
  customerUnit: string
  customerName: string
  brand: string
  unit: string
  catalogPrice: string
  quantity: string
  invoiceTotal: string
  cashback: string
  costDiscount: string
  costUnitPrice: string
  costTotal: string
  saleDiscount: string
  invoiceUnitPrice: string
  grossProfit: string
  remark: string
  paidTime: string
}

export interface CustomerForm {
  unit: string
  contactName: string
  phone: string
  address: string
  remark: string
}

export interface ReturnForm {
  sourceOrderId: string | null
  productId: string | null
  productName: string
  itemNo: string
  returnTime: string
  customerUnit: string
  customerName: string
  brand: string
  unit: string
  catalogPrice: string
  quantity: string
  invoiceTotal: string
  cashback: string
  costDiscount: string
  costUnitPrice: string
  costTotal: string
  saleDiscount: string
  invoiceUnitPrice: string
  grossProfit: string
  remark: string
  paidTime: string
}

export interface DailyStat {
  label: string
  outboundAmount: number
  returnAmount: number
}

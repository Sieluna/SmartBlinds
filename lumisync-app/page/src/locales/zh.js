/**
 * Chinese translations
 * @type {Object}
 */
export const zh = {
  common: {
    loading: '加载中...',
    error: '发生错误',
    success: '成功',
    save: '保存',
    cancel: '取消',
    delete: '删除',
    edit: '编辑',
    add: '添加',
    remove: '移除',
    search: '搜索',
    filter: '筛选',
    apply: '应用',
    reset: '重置',
    back: '返回',
    next: '下一步',
    confirm: '确认',
    submit: '提交',
  },
  validation: {
    required: '此字段为必填项',
    email: '请输入有效的电子邮箱地址',
    password: '密码必须至少8个字符',
    passwordMatch: '密码不匹配',
    integer: '请输入有效的整数',
    number: '请输入有效的数字',
    minLength: '必须至少 {{length}} 个字符',
    maxLength: '不能超过 {{length}} 个字符',
    min: '必须至少 {{min}}',
    max: '不能超过 {{max}}',
    invalidUrl: '请输入有效的URL',
  },
  auth: {
    login: '登录',
    logout: '退出登录',
    register: '注册',
    loginTitle: '欢迎回来',
    loginSubtitle: '输入您的凭据以访问您的账户',
    registerTitle: '创建账户',
    registerSubtitle: '填写您的详细信息以创建新账户',
    email: '电子邮箱',
    password: '密码',
    confirmPassword: '确认密码',
    forgotPassword: '忘记密码？',
    rememberMe: '记住我',
    alreadyAccount: '已有账户？',
    noAccount: '没有账户？',
    loginSuccess: '登录成功',
    logoutSuccess: '退出登录成功',
    registerSuccess: '注册成功',
  },
  navigation: {
    home: '首页',
    dashboard: '仪表盘',
    settings: '设置',
    profile: '个人资料',
    regions: '区域',
    devices: '设备',
    analytics: '分析',
    groups: '群组',
    users: '用户',
    scenes: '场景',
    help: '帮助',
    about: '关于',
  },
  settings: {
    title: '设置',
    appearance: '外观',
    language: '语言',
    theme: '主题',
    notifications: '通知',
    account: '账户',
    security: '安全',
    preferences: '偏好',
    lightTheme: '浅色',
    darkTheme: '深色',
    systemTheme: '系统',
    saveSuccess: '设置保存成功',
  },
  dashboard: {
    welcome: '欢迎，{{name}}',
    summary: '摘要',
    recentActivity: '最近活动',
    quickActions: '快速操作',
    alerts: '警报',
    noData: '无可用数据',
    todaySummary: '今日摘要',
  },
  device: {
    title: '设备',
    name: '设备名称',
    type: '设备类型',
    status: '状态',
    location: '位置',
    temperature: '温度',
    humidity: '湿度',
    illuminance: '光照度',
    position: '位置',
    battery: '电池',
    lastUpdated: '最后更新',
    noDevices: '未找到设备',
    addDevice: '添加设备',
    editDevice: '编辑设备',
    deleteDevice: '删除设备',
    calibrate: '校准',
    emergencyStop: '紧急停止',
    setPosition: '设置位置',
    sensorType: '传感器',
    windowType: '窗户',
  },
  region: {
    title: '区域',
    name: '区域名称',
    devices: '设备',
    noRegions: '未找到区域',
    addRegion: '添加区域',
    editRegion: '编辑区域',
    deleteRegion: '删除区域',
    owner: '所有者',
    visitor: '访客',
  },
  scenes: {
    title: '场景',
    name: '场景名称',
    description: '描述',
    commands: '命令',
    noScenes: '未找到场景',
    addScene: '添加场景',
    editScene: '编辑场景',
    deleteScene: '删除场景',
    activate: '激活',
  },
  errors: {
    notFound: '页面未找到',
    serverError: '服务器错误',
    unauthorized: '未授权',
    forbidden: '禁止访问',
    offline: '您已离线',
    unknown: '未知错误',
  },
  wifi: {
    title: 'WiFi设备配置',
    description: '配置您的智能设备连接到WiFi网络',

    // 网络和状态
    availableNetworks: '可用WiFi网络',
    currentConnection: '当前连接',
    connected: '已连接',

    // 操作按钮
    refresh: '刷新',
    scanning: '扫描中...',
    scanningNetworks: '正在扫描网络...',
    scanPrompt: '点击"刷新"扫描WiFi网络',
    router: 'Router',
    device: 'Device',

    // 配置相关
    configuration: '配置',
    configureDevices: '配置设备',
    selectedDevicesCount: '已选择{{count}}个设备',
    enterRouterPassword: '输入WiFi密码',
    devicePassword: '设备密码',
    deviceEndpoint: '设备URL',

    // 消息提示
    success: {
      devicesConfigured: '{{count}}个设备配置成功！',
    },
    errors: {
      scanFailed: 'WiFi网络扫描失败',
      noRouterSelected: '请先选择路由器',
      configurationFailed: '配置失败：{{error}}',
    },
  },
};

# Class: JsonRpcError

## Extends

- [`IotaHTTPTransportError`](IotaHTTPTransportError.md)

## Constructors

### new JsonRpcError()

> **new JsonRpcError**(`message`, `code`): [`JsonRpcError`](JsonRpcError.md)

#### Parameters

• **message**: `string`

• **code**: `number`

#### Returns

[`JsonRpcError`](JsonRpcError.md)

#### Overrides

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`constructor`](IotaHTTPTransportError.md#constructors)

## Properties

### prepareStackTrace()?

> `static` `optional` **prepareStackTrace**: (`err`, `stackTraces`) => `any`

Optional override for formatting stack traces

#### Parameters

• **err**: `Error`

• **stackTraces**: `CallSite`[]

#### Returns

`any`

#### See

https://v8.dev/docs/stack-trace-api#customizing-stack-traces

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`prepareStackTrace`](IotaHTTPTransportError.md#preparestacktrace)

---

### stackTraceLimit

> `static` **stackTraceLimit**: `number`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`stackTraceLimit`](IotaHTTPTransportError.md#stacktracelimit)

---

### cause?

> `optional` **cause**: `unknown`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`cause`](IotaHTTPTransportError.md#cause)

---

### name

> **name**: `string`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`name`](IotaHTTPTransportError.md#name)

---

### message

> **message**: `string`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`message`](IotaHTTPTransportError.md#message)

---

### stack?

> `optional` **stack**: `string`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`stack`](IotaHTTPTransportError.md#stack)

---

### code

> **code**: `number`

---

### type

> **type**: `string`

## Methods

### captureStackTrace()

> `static` **captureStackTrace**(`targetObject`, `constructorOpt`?): `void`

Create .stack property on a target object

#### Parameters

• **targetObject**: `object`

• **constructorOpt?**: `Function`

#### Returns

`void`

#### Inherited from

[`IotaHTTPTransportError`](IotaHTTPTransportError.md).[`captureStackTrace`](IotaHTTPTransportError.md#capturestacktrace)

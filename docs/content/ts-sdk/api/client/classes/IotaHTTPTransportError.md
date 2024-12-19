# Class: IotaHTTPTransportError

## Extends

- `Error`

## Extended by

- [`IotaHTTPStatusError`](IotaHTTPStatusError.md)
- [`JsonRpcError`](JsonRpcError.md)

## Constructors

### new IotaHTTPTransportError()

> **new IotaHTTPTransportError**(`message`?): [`IotaHTTPTransportError`](IotaHTTPTransportError.md)

#### Parameters

• **message?**: `string`

#### Returns

[`IotaHTTPTransportError`](IotaHTTPTransportError.md)

#### Inherited from

`Error.constructor`

### new IotaHTTPTransportError()

> **new IotaHTTPTransportError**(`message`?, `options`?): [`IotaHTTPTransportError`](IotaHTTPTransportError.md)

#### Parameters

• **message?**: `string`

• **options?**: `ErrorOptions`

#### Returns

[`IotaHTTPTransportError`](IotaHTTPTransportError.md)

#### Inherited from

`Error.constructor`

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

`Error.prepareStackTrace`

---

### stackTraceLimit

> `static` **stackTraceLimit**: `number`

#### Inherited from

`Error.stackTraceLimit`

---

### cause?

> `optional` **cause**: `unknown`

#### Inherited from

`Error.cause`

---

### name

> **name**: `string`

#### Inherited from

`Error.name`

---

### message

> **message**: `string`

#### Inherited from

`Error.message`

---

### stack?

> `optional` **stack**: `string`

#### Inherited from

`Error.stack`

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

`Error.captureStackTrace`

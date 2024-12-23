# Class: FaucetRateLimitError

## Extends

- `Error`

## Constructors

### new FaucetRateLimitError()

> **new FaucetRateLimitError**(`message`?): [`FaucetRateLimitError`](FaucetRateLimitError.md)

#### Parameters

• **message?**: `string`

#### Returns

[`FaucetRateLimitError`](FaucetRateLimitError.md)

#### Inherited from

`Error.constructor`

### new FaucetRateLimitError()

> **new FaucetRateLimitError**(`message`?, `options`?): [`FaucetRateLimitError`](FaucetRateLimitError.md)

#### Parameters

• **message?**: `string`

• **options?**: `ErrorOptions`

#### Returns

[`FaucetRateLimitError`](FaucetRateLimitError.md)

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

### calc slot
```js
var epochIndex = (await api.query.babe.epochIndex()).toNumber();
var currentSlot = (await api.query.babe.currentSlot()).toNumber();
var genesisSlot = (await api.query.babe.genesisSlot()).toNumber();
var diff = currentSlot - ((epochIndex * 120) + genesisSlot);
diff
```

``` rust
let diff = CurrentSlot::get().saturating_sub(Self::current_epoch_start());
(EpochIndex::get() * T::EpochDuration::get()) + GenesisSlot::get()
diff > EpochDuration (120)
```

327094555 - (317142376 + 9952080)

## Block investigation on Nikau

```
> block.toJSON()
{
  parentHash: '0x18d568b4beec9dfad6178221166fb3fc58f0b9497c1ec9cfb1de8910623d1e4a',
  number: 1805572,
  stateRoot: '0x724f3bca4388f6c6a634bc53943a7a68f18a0ef3849440080c938154e8bb621a',
  extrinsicsRoot: '0x3bce50d78acc7105e074f126e245422b4ebc19a839329eb2d35dd0a508c1666f',
}
```

### When Nikau is at block: 1805571
```js
var epochIndex = (await api.query.babe.epochIndex.at("0x18d568b4beec9dfad6178221166fb3fc58f0b9497c1ec9cfb1de8910623d1e4a")).toNumber();
var currentSlot = (await api.query.babe.currentSlot.at("0x18d568b4beec9dfad6178221166fb3fc58f0b9497c1ec9cfb1de8910623d1e4a")).toNumber();
var genesisSlot = (await api.query.babe.genesisSlot.at("0x18d568b4beec9dfad6178221166fb3fc58f0b9497c1ec9cfb1de8910623d1e4a")).toNumber();
var diff = currentSlot - ((epochIndex * 120) + genesisSlot);
diff
```
slot number is `22`

### When Nikau is at block: 1805572 (latest)
0xe8f34606000b383f0b65338459c3c538b786f5f8eef108d26f56a272e08f6f82
0x9a1a8b516d0efce8edef98b94aba6d8b09918a3cf6dabbc093224b48622d3870

```js
> var epochIndex = (await api.query.babe.epochIndex()).toNumber();
> var currentSlot = (await api.query.babe.currentSlot()).toNumber();
> var genesisSlot = (await api.query.babe.genesisSlot()).toNumber();
> var diff = currentSlot - ((epochIndex * 120) + genesisSlot);
> diff
```
slot number is `3117`
this block is forcing a new epoch unexpectedly
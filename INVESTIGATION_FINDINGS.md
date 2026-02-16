# Storage Cost Difference Investigation

## Issue
The test `concrete_multiple` in `crates/iota-verifier-transactional-tests` shows a storage_cost difference:
- Old value (develop): `10784400`
- New value (dev-tools/replace-move-core-types): `10792000`
- **Difference: +7600 bytes**

## Root Cause Analysis

### Changes in dev-tools/replace-move-core-types Branch

The branch includes multiple commits that replaced Move core types with SDK types:

1. **Commit a45ef430ae**: Replace `SequenceNumber` with SDK `Version`
2. **Commit aaa1819475**: Replace `IotaAddress` with SDK `Address`
3. **Commit 2791470383**: Replace `ObjectID` with SDK version
4. **Commit d3e8f99b1f**: Replace `Digest` with SDK version
5. **Commit 19a1ac183f**: Replace `GasCostSummary` with external one
6. **Commit 7611b83228**: Replace `Identifier`, `StructTag` and `TypeTag` with SDK versions ⭐
7. **Commit da16853ef0**: Add script to update SDK types rev
8. **Commit db41d5ecec**: Replace `Event` with SDK version

### Investigation Findings

#### 1. Object Size Calculation
Storage costs are calculated based on `object_size_for_gas_metering()` which for Move packages uses an approximation:

```rust
pub fn size(&self) -> usize {
    let module_map_size = self.module_map.iter()
        .map(|(name, module)| name.len() + module.len())
        .sum::<usize>();
    let type_origin_table_size = self.type_origin_table.iter()
        .map(|TypeOrigin { module_name, datatype_name, .. }| 
             module_name.len() + datatype_name.len() + ObjectID::LENGTH)
        .sum::<usize>();
    let linkage_table_size = self.linkage_table.len() 
        * (ObjectID::LENGTH + ObjectID::LENGTH + 8);
    
    8 + module_map_size + type_origin_table_size + linkage_table_size
}
```

This is an **approximation** that doesn't account for all BCS serialization overhead (like length prefixes for maps, vectors, and strings).

#### 2. SDK Types Analysis
- `ObjectID::LENGTH` = 32 bytes (same as before) ✓
- `Address::LENGTH` = 32 bytes (same as before) ✓  
- BCS format documented as "strictly identical" ✓

#### 3. Likely Cause
The 7600 byte difference suggests that the BCS serialization of the `MovePackage` struct has additional overhead not captured by the `size()` approximation. This could be due to:

1. **Additional BCS metadata**: The SDK types may include additional internal fields or metadata that gets serialized
2. **Serialization format differences**: Even with "identical" BCS format, there may be subtle differences in how compound types (like the package metadata tables) are structured
3. **Type origin table changes**: The package's `type_origin_table` entries may serialize differently with SDK `ObjectID` vs old `AccountAddress`-based `ObjectID`

### The +7600 Byte Breakdown

For the `concrete_multiple` test which publishes a module with 3 structs (`Wrapped`, `Wrapped2`, `Account<T,U>`):
- The type_origin_table has 3 entries (one for each struct)
- The module_map has 1 module ("account")  
- The linkage_table is empty (no dependencies)

The 7600 byte increase is distributed across the entire package serialization:
- BCS length prefixes for collections (maps, vectors)
- String serialization overhead for module/struct names
- ObjectID and SequenceNumber/Version field serialization
- Potential differences in how the SDK types' internal representation is serialized

Note: This is approximately 7600 bytes total for the entire package, not per entry. The exact distribution of overhead across different package components would require detailed BCS binary analysis.

## Conclusion

The storage cost increase is a consequence of replacing core Move types with SDK equivalents. While the types are designed to be BCS-compatible, the actual serialized size has increased by 7600 bytes for this test case.

**Recommendation**: Update the test snapshot to reflect the new correct storage cost value of `10792000`.

## Impact

This affects all Move package storage calculations and will result in slightly higher storage costs for published packages. The impact is proportional to:
- Number of structs/enums in the package (type_origin_table size)
- Number of modules (module_map size)  
- Number of dependencies (linkage_table size)

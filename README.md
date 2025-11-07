# Elias shop MariaDB to Saleor GQL Migration

This collection of tools takes products from Elias' MariaDB directly and uploads them all directly to Saleor through GQL. I'm making this code public because it can serve well as a starting point for other people migrating from OpenCart or Prestashop to Saleor.

To make use of this repo, you WILL have to modify or most likely completely rewrite `./src/get_sqls.rs` to fit this app to your database and formatting. Elias shop is not something commonly found in the wild so might be doing things in a weird way,
but still should serve as a good reference or starting point.

To make this process bit less painless I assumed a few things:

1. All products have only 1 (default) variant
2. All products are in a single warehouse
3. All products are in a single channel

Also, since the concept of a "product type" wasn't present in our old eshop, I had created another tool that had dumped all the categories into a yaml file (example in `./filled_out_kategorie.yaml`)
where an employee matched the category name to a new product type that would be created,
and all products beloning under that category (under any level, unless overwritten by an immediate parent) were assigned to that product type.

Pictures belong in `./media/products` (I think) and their paths and names are taken from some database relationship row thingy

Old database I served from `./db` through docker compose, had a single .sql file dump of the previous shop and I queried from there

There's also a tool that just deletes all products, product types and categories in `./wipe-products/`.
This is so I can set up channels, warehouses and tax classes once and if something had gone wrong during product upload I didn't have to nuke the DB and reconfigure all that.

# License and contributions

I haven't touched this code for years, and it was not only held together by but also created from tape and WD-40. Apologies for anyone struggling to read this and having to pull their hairs out,
this was one of my first big rust projects xd
code is under aGPL-3.0, so please if you write a migration tool on the basis of this code for other eshops (wordpress, opencart, prestashop etc..) make a PR and I'll add it as another branch. main will stay as is.

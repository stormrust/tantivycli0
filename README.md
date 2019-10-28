
This is a fork of
https://github.com/tantivy-search/tantivy-cli

```
cp ./data/wiki-articles-1000.json /tmp/tantivy
cd /tmp/tantivy
mkdir wiki
cp ./data/meta.json ./wiki
tanc index --file wiki-articles-1000.json --index ./wiki
```

See [the data dir](./data/Readme.md) for more details...   
See [the doc dir](./doc/README.md) for more details...

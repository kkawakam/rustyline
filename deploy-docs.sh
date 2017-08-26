#!/bin/bash

rev=$(git rev-parse --short HEAD)

cd target/doc

echo '<meta http-equiv=refresh content=0;url=rustyline/index.html>' > index.html

git init
git config user.name "Katsu Kawakami"
git config user.email "kkawa1570@gmail.com"

git remote add upstream "https://$GH_TOKEN@github.com/kkawakam/rustyline.git"
git fetch upstream 
git push origin --delete gh_pages > /dev/null 2>&1

touch .

git add -A .
git commit -m "rebuild pages at ${rev}"
git push -f -q upstream gh-pages > /dev/null 2>&1

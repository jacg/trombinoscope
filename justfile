# -*-Makefile-*-

serve PORT='8080':
     dx serve --hot-reload --port {{PORT}}

publish:
	dx build --release
	cp docs/{index,404}.html
	git add docs
	git commit -m "publish"
	git push --force-with-lease origin publish

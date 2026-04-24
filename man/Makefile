PREFIX ?= /usr/local
MAN_DIR ?= $(PREFIX)/share/man/man1

MAN_PAGES = $(wildcard *.1)

install:
	install -d $(DESTDIR)$(MAN_DIR)
	install -m 644 $(MAN_PAGES) $(DESTDIR)$(MAN_DIR)

uninstall:
	cd $(DESTDIR)$(MAN_DIR) && rm -f $(MAN_PAGES)

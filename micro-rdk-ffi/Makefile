TARGET = example/example

OBJS = example/example.o

LPATH="$(PWD)/../target/debug"
LDFLAGS = -L$(LPATH) -Wl,-rpath,$(LPATH)
LDFLAGS += `pkg-config --libs openssl`
LIBS= -lmicrordk -lm
INCLUDE=-I./include/

all: header $(TARGET)

%.o: %.c
	$(CC) -c -g -o $@ $< $(INCLUDE)

$(TARGET): $(OBJS)
	$(CC) -g -o $(TARGET) $(OBJS) $(LDFLAGS) $(LIBS)

header:
	cbindgen --config cbindgen.toml --output include/micrordk.h

clean:
	rm -f $(OBJS) $(TARGET)

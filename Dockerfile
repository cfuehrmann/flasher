FROM nginx:alpine
RUN mkdir /usr/share/nginx/html/flasher
COPY build /usr/share/nginx/html/flasher
COPY default.conf /etc/nginx/conf.d

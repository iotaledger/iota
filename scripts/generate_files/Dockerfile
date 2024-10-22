FROM node:20-bookworm

# Get USER_ID and GROUP_ID from build-args
ARG USER_ID=1000
ARG GROUP_ID=1000

# Check if the USER_ID or GROUP_ID need to be updated, and adjust permissions accordingly
RUN if [ $GROUP_ID -ne 1000 ]; then \
    groupmod -g $GROUP_ID node; \
fi && \
if [ $USER_ID -ne 1000 ]; then \
    usermod -u $USER_ID node; \
fi && \
if [ $USER_ID -ne 1000 ] || [ $GROUP_ID -ne 1000 ]; then \
    chown -R $USER_ID:$GROUP_ID /home/node; \
fi

# Install pnpm
RUN npm install -g pnpm

USER node
WORKDIR /home/node

# Set the safe.directory to /app
RUN git config --global --add safe.directory /app

RUN mkdir -p /home/node/app

WORKDIR /home/node/app
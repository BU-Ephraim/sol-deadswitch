const anchor = require("@coral-xyz/anchor");

module.exports = async function deploy(provider) {
  anchor.setProvider(provider);
};

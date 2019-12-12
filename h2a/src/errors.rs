use amiquip;
use actix::prelude::{SendError};
use actix::{MailboxError};
use actix_web::{
    error,
    Error as AWError, 
    ResponseError,
    HttpResponse,
};

use derive_more::{Display}; // naming it clearly for illustration purposes

use crate::messages::{AmqpMessage};

#[derive(Fail, Debug, Display, Clone)]
pub enum SvcError {
    #[display(fmt = "Operation not allowed in current state: {}", _0)]
    InvalidOperationForState(String),
    #[display(fmt = "Critical error - restart service with valid settings: {}", _0)]
    NonRecoverableError(String),
    #[display(fmt = "Connection error: {}", _0)]
    ConnectionError(String),
    #[display(fmt = "Channel error: {}", _0)]
    ChannelError(String),
    #[display(fmt = "Message error: {}", _0)]
    MessageError(String),

    #[display(fmt = "Exchange/queue not found: {}", _0)]
    TargetNotFound(String),

    #[display(fmt = "Generic AMQP error: {:?}", _0)]
    GenericAmiquipError(String),

    #[display(fmt = "Local queue error")]
    ActixQueueOverflow,
    #[display(fmt = "Send error")]
    ActixSendError,


    // #[display(fmt = "Json error")]

    // SerdeJson,
    // #[display(fmt = "Recursive Routing")]
    // RecurrentRouting,
    // #[display(fmt = "Lua Error")]
    // LuaError(String),
    // #[display(fmt = "Unknown Error")]
    // UnknownError(String),
}

impl From<MailboxError> for SvcError {
    fn from(_: MailboxError) -> Self {
        SvcError::ActixQueueOverflow
    }
}

impl From<SendError<AmqpMessage>> for SvcError {
    fn from(_: SendError<AmqpMessage>) -> Self {
        SvcError::ActixQueueOverflow
    }
}

impl From<amiquip::Error> for SvcError {
    fn from(error: amiquip::Error) -> Self {
        use amiquip::Error::*;
        // SvcError::GenericAmiquipError(error)
        let msg = format!("{}", error);
        match error {
        // 	The client closed the channel.	"channel has been closed"	
        ClientClosedChannel => SvcError::ChannelError(msg),
        // 	The server sent us a consumer tag that is equal to another consumer tag we already have on the same channel.	"server sent duplicate consumer tag for channel {}: {}"	
        DuplicateConsumerTag { .. } => SvcError::ChannelError(msg),
        // 	The I/O loop has dropped the sending side of a channel, typically because it has exited due to another error.	"i/o loop dropped sending side of a channel"	
        EventLoopDropped => SvcError::ChannelError(msg),
        // 	Forking the I/O thread failed.	"fork failed: {}"	
        ForkFailed { .. } => SvcError::ChannelError(msg),
        // 	The I/O thread panicked.	"I/O thread panicked"	
        IoThreadPanic => SvcError::ChannelError(msg),
        // 	The server sent frames for a channel ID we don't know about.	"received message for nonexistent channel {}"	
        ReceivedFrameWithBogusChannelId { .. } => SvcError::ChannelError(msg),
        // 	The server closed the given channel with the given reply code and text.	"server closed channel {} (code={}, message={})"	
        ServerClosedChannel { .. } => SvcError::ChannelError(msg),
        // 	An explicit channel ID was requested, but that channel is unavailable for use (e.g., because there is another open channel with the same ID).	"requested channel id ({}) is unavailable"	
        UnavailableChannelId { .. } => SvcError::ChannelError(msg),
        // 	The server sent us a [`Delivery`](struct.Delivery.html) for a channel we don't know about.	"received delivery with unknown consumer tag for channel {}: {}"	
        UnknownConsumerTag { .. } => SvcError::ChannelError(msg),
        // 	The client closed the connection.	"client closed connection"	
        ClientClosedConnection => SvcError::ConnectionError(msg),
        // 	Timeout occurred while performing the initial TCP connection.	"timeout occurred while waiting for TCP connection"	
        ConnectionTimeout => SvcError::ConnectionError(msg),
        // 	Could not create mio Poll handle.	"failed to create polling handle: {}"	
        CreatePollHandle { .. } => SvcError::ConnectionError(msg),
        // 	Could not register descriptor with Poll handle.	"failed to deregister object with polling handle: {}"	
        DeregisterWithPollHandle { .. } => SvcError::ConnectionError(msg),
        // 	The I/O loop attempted to send a message to a caller that did not exist. This indicates either a bug in amiquip or a connection that is in a bad state and in the process of tearing down.	"i/o loop thread tried to communicate with a nonexistent client"	
        EventLoopClientDropped => SvcError::ConnectionError(msg),
        // 	Failed to open TCP connection.	"failed to connect to {}: {}"	
        FailedToConnect { .. } => SvcError::ConnectionError(msg),
        // 	Failed to poll mio Poll handle.	"failed to poll: {}"	
        FailedToPoll { .. } => SvcError::ConnectionError(msg),
        // 	The requested frame size is smaller than the minimum required by AMQP.	"requested frame max is too small (min = {}, requested = {})"	
        FrameMaxTooSmall { .. } => SvcError::ConnectionError(msg),
        // 	An I/O error occurred while reading from the socket.	"I/O error while reading socket: {}"	
        IoErrorReadingSocket { .. } => SvcError::ConnectionError(msg),
        // 	An I/O error occurred while writing from the socket.	"I/O error while writing socket: {}"	
        IoErrorWritingSocket { .. } => SvcError::ConnectionError(msg),
        // 	The server missed too many successive heartbeats.	"missed heartbeats from server"	
        MissedServerHeartbeats => SvcError::ConnectionError(msg),
        // 	Could not register descriptor with Poll handle.	"failed to register object with polling handle: {}"	
        RegisterWithPollHandle { .. } => SvcError::ConnectionError(msg),
        // 	Error resolving a URL into an IP address (or addresses).	"failed to resolve IP address of {}: {}"	
        ResolveUrlToSocketAddr { .. } => SvcError::ConnectionError(msg),
        // 	The server closed the connection with the given reply code and text.	"server closed connection (code={} message={})"	
        // ServerClosedConnection { code, message } => SvcError::ConnectionError(msg),
        // 	The underlying socket was closed.	"underlying socket closed unexpectedly"	
        UnexpectedSocketClose => SvcError::ConnectionError(msg),
        // 	Failed to resolve a URL into an IP address (or addresses).	"URL did not resolve to an IP address: {}"	
        UrlNoSocketAddrs { .. } => SvcError::ConnectionError(msg),
        // 	Could not parse heartbeat parameter of URL.	"could not parse heartbeat parameter of URL {}: {}"	
        UrlParseHeartbeat { .. } => SvcError::ConnectionError(msg),
        // 	The client sent an AMQP exception to the server and closed the connection.	"internal client exception - received unhandled frames from server"	
        ClientException => SvcError::MessageError(msg),
        // 	No more channels can be opened because there are already [`channel_max`](struct.ConnectionOptions.html#method.channel_max) channels open.	"no more channel ids are available"	
        ExhaustedChannelIds => SvcError::MessageError(msg),
        // 	We received a valid AMQP frame but not one we expected; e.g., receiving an incorrect response to an AMQP method call.	"AMQP protocol error - received unexpected frame"	
        FrameUnexpected => SvcError::MessageError(msg),
        // 	We received data that could not be parsed as an AMQP frame.	"received malformed data - expected AMQP frame"	
        MalformedFrame => SvcError::MessageError(msg),
        // Error from underlying TLS implementation.	"could not create TLS connector: {}"	
        #[cfg(feature = "native-tls")]
        CreateTlsConnector { .. } => SvcError::NonRecoverableError(msg),
        // 	URL contains extra path segments.	"URL contains extraneous path segments: {}"	
        ExtraUrlPathSegments { .. } => SvcError::NonRecoverableError(msg),
        // 	An insecure URL was supplied to [`Connection::open`](struct.Connection.html#method.open), which only allows `amqps://...` URLs.	"insecure URL passed to method that only allows secure connections: {}"	
        InsecureUrl { .. } => SvcError::NonRecoverableError(msg),
        // 	The supplied authentication credentials were not accepted by the server.	"invalid credentials"	
        InvalidCredentials => SvcError::NonRecoverableError(msg),
        // 	Invalid scheme for URL.	"invalid scheme for URL {}: expected `amqp` or `amqps`"	
        InvalidUrlScheme { .. } => SvcError::NonRecoverableError(msg),
        // 	The server requested a Secure/Secure-Ok exchange, which are currently unsupported.	"SASL secure/secure-ok exchanges are not supported"	
        SaslSecureNotSupported => SvcError::NonRecoverableError(msg),
        // 	Failed to set the port on a URL.	"cannot specify port for URL {}"	
        SpecifyUrlPort { .. } => SvcError::NonRecoverableError(msg),
        // 	A TLS connection was requested (e.g., via URL), but the amiquip was built without TLS support.	"amiquip built without TLS support"	
        TlsFeatureNotEnabled => SvcError::NonRecoverableError(msg),
        // cfg(feature = "native-tls")]	The TLS handshake failed.	"TLS handshake failed: {}"	
        #[cfg(feature = "native-tls")]
        TlsHandshake { .. } => SvcError::NonRecoverableError(msg),
        // 	The server does not support the requested auth mechanism.	"requested auth mechanism unavailable (available = {}, requested = {})"	
        UnsupportedAuthMechanism { .. } => SvcError::NonRecoverableError(msg),
        // 	The server does not support the requested locale.	"requested locale unavailable (available = {}, requested = {})"	
        UnsupportedLocale { .. } => SvcError::NonRecoverableError(msg),
        // 	Invalid auth mechanism requested in URL.	"invalid auth mechanism for URL {}: {} (expected `external`)"	
        UrlInvalidAuthMechanism { .. } => SvcError::NonRecoverableError(msg),
        // 	URL is missing domain.	"invalid URL (missing domain): {}"	
        UrlMissingDomain { .. } => SvcError::NonRecoverableError(msg),
        // 	Could not parse channel_max parameter of URL.	"could not parse channel_max parameter of URL {}: {}"	
        UrlParseChannelMax { .. } => SvcError::NonRecoverableError(msg),
        // 	Could not parse connection_timeout parameter of URL.	"could not parse connection_timeout parameter of URL {}: {}"	
        UrlParseConnectionTimeout { .. } => SvcError::NonRecoverableError(msg),
        // 	URL parsing failed.	"could not parse url: {}"	
        UrlParseError { .. } => SvcError::NonRecoverableError(msg),
        // 	Unsupported URL parameter.	"unsupported parameter in URL {}: {}"	
        UrlUnsupportedParameter { .. } => SvcError::NonRecoverableError(msg),
            // These cases means service has critical errors and cannot be connected to backend in any case  !!!
            // UrlParseError { source } => SvcError::NonRecoverableError(msg),
            // TlsFeatureNotEnabled => SvcError::NonRecoverableError(msg),
            // InsecureUrl { url } => SvcError::NonRecoverableError(msg),
            // SpecifyUrlPort { url } => SvcError::NonRecoverableError(msg),
            // InvalidUrlScheme { url } => SvcError::NonRecoverableError(msg),
            // ExtraUrlPathSegments { url } => SvcError::NonRecoverableError(msg),
            // UrlParseHeartbeat { url, source } => SvcError::NonRecoverableError(msg),
            // UrlInvalidAuthMechanism { url, mechanism } => SvcError::NonRecoverableError(msg),
            // UrlUnsupportedParameter { url, parameter } => SvcError::NonRecoverableError(msg),
            // #[cfg(feature = "native-tls")]
            // TlsHandshake { source } => SvcError::NonRecoverableError(msg),
            // #[cfg(feature = "native-tls")]
            // CreateTlsConnector { source } => SvcError::NonRecoverableError(msg),
            // UnsupportedAuthMechanism {available, requested} => SvcError::NonRecoverableError(msg),
            // UnsupportedLocale {available, requested} => SvcError::NonRecoverableError(msg),
            // SaslSecureNotSupported => SvcError::NonRecoverableError(msg),
            // InvalidCredentials => SvcError::NonRecoverableError(msg),
            // SaslSecureNotSupported => SvcError::NonRecoverableError(msg),

            // Errors on specific message only, connection is functional

            // Errors requires re-connection



            ServerClosedChannel{code, ..} => {
                if code == 404 {
                    SvcError::TargetNotFound(msg)
                } else  {
                    SvcError::GenericAmiquipError(msg)
                }
            },
            _ => SvcError::GenericAmiquipError(msg)
        }



    }

}


#include <lib/std.mi>
#include "../../../../widgets/cprowidget.mi"
#include <lib/pldir.mi>
#include <lib/fileio.mi>
#include <lib/application.mi>
#include <lib/colormgr.mi>
#include "convert_address.mi"

Function resizeResults(int items);
Function addProvider(list paramname, list paramvalue);
Function String replaceString(string baseString, string toreplace, string replacedby);
Function int searchInListForItem(String input);
Function selectSaved();
Function surfSelected();
Function initLoadFiles();
Function updateDDList();
Function ddlOpenClose();

//Browser Stuff
Function Boolean isUrl (String t);
Function string prepareWebString(string url, string replace);
Function string strReplace(string str, string replace, string by);


Global Container results_container;
Global Layout results_layout;

Global Group myGroup, ddlist, gBarAuto, gBarMan;
Global GuiList myList;
Global XmlDoc myDoc;
Global List loaded_P_Names, loaded_P_Url, loaded_P_Icons;
Global int sourceNo, h_tune, tempI;
Global boolean onetime, continueLoad, skipForwardHide, cooldowned;
Global Text ddBoxText;
Global ToggleButton enabledSwitch;
Global Button menuOptions, dropListButton;
Global PopUpMenu popMenu;
Global GuiObject browserXUI, fakeSB; // ddbbg;
Global Browser myBrowser;
Global Layer ddlMouseT, ddlIcon, lBarAuto, lBarMan;
Global Button bback, bffwd, brefresh, bstop, bmark;
Global Togglebutton modeToggle;
Global Edit urlbox;
Global String firstURL, lastURL, lastestURL;
Global PopUpMenu popMenu;

Global ColorMgr cm;

Global Timer focus_callback, cooldown;

System.onScriptLoaded(){
	//setPublicInt("ClassicPro.BrowserPro.loaded", 1);
	initWidget();
	
	results_container = newDynamicContainer("browserpro");
	results_layout = results_container.getLayout("normal");

	myGroup = getScriptGroup();
	myList = results_layout.findObject("BrowserPro.list");
	ddBoxText = myGroup.findObject("browserpro.ddl.text");
	menuOptions = myGroup.findObject("cpro.widget.browserpro.options");
	dropListButton = myGroup.findObject("dropdownlist.button");
	fakeSB = myGroup.findObject("browserpro.ddlist");
	myBrowser = myGroup.findObject("browserpro.browser");
	ddlMouseT = myGroup.findObject("browserpro.ddl.mousetrap");
	ddlIcon = myGroup.findObject("browserpro.ddl.icon");
	bback = myGroup.findObject("browser.back");
	bffwd = myGroup.findObject("browser.fwd");
	brefresh = myGroup.findObject("browser.refresh");
	bstop = myGroup.findObject("browser.stop");
	modeToggle = myGroup.findObject("reader.mode");
	urlbox = myGroup.findObject("browserpro.ddl.editbox");
	//ddbbg = myGroup.findObject("browserpro.ddl.rect");
	bmark = myGroup.findObject("reader.bookmark");
	ddlist = myGroup.findObject("browserpro.ddlist.group");
	gBarAuto = myGroup.findObject("centro.reader.bar.auto");
	gBarMan = myGroup.findObject("centro.reader.bar.man");
	lBarAuto = myGroup.findObject("centro.reader.bar.auto.busy.0");
	lBarMan = myGroup.findObject("centro.reader.bar.auto.busy.1");

	focus_callback = new Timer;
	focus_callback.setDelay(100);

	cooldown = new Timer;
	cooldown.setDelay(500);

	cm = new ColorMgr;

	myList.setIconWidth(16);
	myList.setShowIcons(1);
	sourceNo=0;
	myBrowser.setCancelIEErrorPage(true);
	
	//modeToggle.setActivated(getPublicInt("ClassicPro.Reader.Mode", 1));
	//modeToggle.onToggle(getPublicInt("ClassicPro.Reader.Mode", 1));
}

myGroup.onSetVisible(boolean onOff){
	if(onOff){
		firstURL="";
		lastURL="";
		skipForwardHide=false;
		bback.setXMLParam("ghost", "1");
		bback.setXMLParam("image", "cpro2.reader.back.0");
		bffwd.setXMLParam("ghost", "1");
		bffwd.setXMLParam("image", "cpro2.reader.forward.0");

		initLoadFiles();
		if(continueLoad) surfSelected();
		else{
			myGroup.hide();
			debug("Something went wrong!");
		}
	}
}

cm.onColorThemeChanged(String newtheme)
{
	if (onCTChangeReload)
	{
		surfSelected();
	}
}

System.onScriptUnloading (){
	//setPublicInt("ClassicPro.BrowserPro.loaded", 0);
	myGroup.hide();
	focus_callback.stop();
	delete loaded_P_Names;
	delete loaded_P_Url;
	delete focus_callback;
	delete cm;
}


initLoadFiles(){
	if(!onetime){
		loaded_P_Names = new List;
		loaded_P_Url = new List;
		loaded_P_Icons = new List;
		
		myDoc = new XmlDoc;
		String temp = Application.GetApplicationPath()+"\Plugins\ClassicPro\engine\xui\CentroSUI\_v2\Reader\source\_"+strlower(System.getLanguageId())+".xml";
		myDoc.load (temp);
		if(!myDoc.exists()) temp = Application.GetApplicationPath()+"\Plugins\ClassicPro\engine\xui\CentroSUI\_v2\Reader\source\_en-us.xml";
		myDoc.load (temp);
		if(!myDoc.exists()){
			debug("No source file found! Please make sure you installed ClassicPro correct.");
			return;
		}
		
		continueLoad=true;
		

		myDoc.parser_addCallback("WasabiXML/BrowserPro/*");
		myDoc.parser_start();
		myDoc.parser_destroy();
		delete myDoc;

		onetime=true;

		selectSaved();
	}
}

addProvider(list paramname, list paramvalue){
	myList.addItem("Read error");
	for(int i=0; i<paramname.getNumItems(); i++){
		if(strlower(paramname.enumItem(i))=="name"){
			myList.setItemLabel(sourceNo, paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="comment"){
			myList.setSubItem(sourceNo, 1, paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="url"){
			loaded_P_Url.addItem(paramvalue.enumItem(i));
		}
		else if(strlower(paramname.enumItem(i))=="icon"){
			myList.setItemIcon(sourceNo, paramvalue.enumItem(i));
			loaded_P_Icons.addItem(paramvalue.enumItem(i));
		}		
	}
	loaded_P_Names.addItem(myList.getItemLabel(sourceNo, 0));
	sourceNo++;
}

myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	if(strlower(xmltag) == "sourceitem"){
		addProvider(paramname, paramvalue);
	}
}

resizeResults(int items){
	if(items>22) items=22;
	if(items>1) h_tune=28;
	else h_tune=19;

	results_layout.setXmlParam("h", integerToString(h_tune+items*17+1));
}

dropListButton.onLeftClick(){
	ddlOpenClose();
}
ddlMouseT.onLeftButtonUp(int x, int y){
	ddlOpenClose();
}
ddlMouseT.onRightButtonUp(int x, int y){
	/*popMenu = new PopUpMenu;
	popMenu.addCommand("Open Current Website in your Default Browser", 1, 0, 0);
	int result = popMenu.popAtMouse();
	if(result==1) System.navigateUrl(lastestURL);
	
	delete popMenu;*/
	
	System.navigateUrl(lastestURL);
	complete;
}

ddlOpenClose(){
	if(results_layout.isVisible()) results_layout.hide();
	else{
		if(modeToggle.getActivated()) tempI = 24;
		else tempI = 0;
		
		results_layout.setXmlParam("x", integerToString(fakeSB.clientToScreenX(fakeSB.getLeft())-1-tempI));
		results_layout.setXmlParam("y", integerToString(fakeSB.clientToScreenY(fakeSB.getTop() + fakeSB.getHeight())-4));
		results_layout.setXmlParam("w", integerToString(fakeSB.getWidth()+2+tempI));
	
		results_layout.show();
		resizeResults(loaded_P_Names.getNumItems());
		results_layout.setXmlParam("ontop", "1"); //dunno
		results_layout.setfocus();
		focus_callback.start();
	}
}

focus_callback.onTimer ()
{
	if (!results_layout.isActive())
	{
		focus_callback.stop();
		if (!results_layout) return;
		results_layout.hide();
	}
}

selectSaved(){
	if(!onetime) return;
	int i = searchInListForItem(getPublicString("ClassicPro.BrowserPro", "0"));
	//if(getPublicString("ClassicPro.BrowserPro", "0")) setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(0, 0)); //wtf was this???? forgot :P
	myList.setSelected(i,1);
	updateDDList();
}
searchInListForItem(String input){
	for(int i=0; i<myList.getNumItems();i++){
		if(input == myList.getItemLabel(i, 0)) return i;
	}
	
	setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(0, 0)); //if nothing was found
	return 0;
}
surfSelected(){
	if(!onetime) return;
	String myUrl = loaded_P_Url.enumItem(loaded_P_Names.findItem(getPublicString("ClassicPro.BrowserPro", "0")));
	//autoclick=true;
	myBrowser.navigateUrl(prepareCustomUrl(myUrl));
	//urlbox.setText(prepareCustomUrl(myUrl));
}
System.onTitleChange(String newtitle){
	if(myGroup.isVisible()){
		if(!modeToggle.getActivated()){
			cooldowned=false; //if user skips track just when finished load call was received (and cooldown not complete)
			surfSelected();
		}
	}
}
myList.onLeftClick(Int itemnum){
	setPublicString("ClassicPro.BrowserPro", myList.getItemLabel(itemnum, 0));
	surfSelected();
	results_layout.hide();
	updateDDList();
}

updateDDList(){
	if(!onetime) return;
	ddBoxText.setText(getPublicString("ClassicPro.BrowserPro", "0"));
	String iconsID = loaded_P_Icons.enumItem(loaded_P_Names.findItem(getPublicString("ClassicPro.BrowserPro", "0")));
	if(!modeToggle.getActivated()) ddlIcon.setXmlParam("image", iconsID);
}

modeToggle.onToggle(Boolean onOff){
	setPublicInt("ClassicPro.Reader.Mode", onOff);
	//debugint(getPublicInt("ClassicPro.Reader.Mode", 3));
	//if(repBut.setCurCfgVal()==0) updateInfo("Repeat: Off");

	if(onOff){
		ddBoxText.hide();
		dropListButton.hide();
		ddlMouseT.hide();
		urlbox.show();
		//ddbbg.setXmlParam("w", "-24");
		ddlIcon.setXmlParam("image", "icon.readingmode");

		gBarAuto.hide();
		gBarMan.show();
		
		//grid.setXmlParam("topleft", "cpro2.reader.bg.leftlong");
		ddlist.setXmlParam("x", "104");
		ddlist.setXmlParam("w", "-130");
		bmark.show();

	}
	else{
		ddBoxText.show();
		dropListButton.show();
		ddlMouseT.show();
		urlbox.hide();
		surfSelected();
		//ddbbg.setXmlParam("w", "-39");
		String iconsID = loaded_P_Icons.enumItem(loaded_P_Names.findItem(getPublicString("ClassicPro.BrowserPro", "0")));
		ddlIcon.setXmlParam("image", iconsID);

		gBarAuto.show();
		gBarMan.hide();

		//grid.setXmlParam("topleft", "cpro2.reader.bg.left");
		ddlist.setXmlParam("x", "80");
		ddlist.setXmlParam("w", "-106");
		bmark.hide();

	}
}

bmark.onLeftClick(){
	dropListButton.leftClick();
}


//----------------------------------------------------------------------------------------------------------------
// Browser main controls.
//----------------------------------------------------------------------------------------------------------------

bback.onLeftClick(){
	if(bffwd.getXMLParam("ghost")=="1") lastURL=lastestURL;
	
	bffwd.setXMLParam("ghost", "0");
	bffwd.setXMLParam("image", "cpro2.reader.forward.1");
	
	skipForwardHide=true;
	myBrowser.back();
}
bffwd.onLeftClick(){
	skipForwardHide=true;
	myBrowser.forward();
}
brefresh.onLeftClick(){
	surfSelected();
}
bstop.onLeftClick(){
	myBrowser.stop();
	bstop.hide();
	brefresh.show();
}

bstop.onSetVisible(Boolean onoff){
	lBarAuto.cancelTarget();
	lBarMan.cancelTarget();

	if(onoff){
		lBarAuto.setTargetA(255);
		lBarMan.setTargetA(255);
		lBarAuto.setTargetSpeed(0.2);
		lBarMan.setTargetSpeed(0.2);
	}
	else{
		lBarAuto.setTargetA(0);
		lBarMan.setTargetA(0);
		lBarAuto.setTargetSpeed(0.6);
		lBarMan.setTargetSpeed(0.6);
	}
	lBarAuto.gotoTarget();
	lBarMan.gotoTarget();
}

/////////////////////////////////////

urlbox.onEnter ()
{
	string t = urlbox.getText();

	if (t == "")
		return;
	
	if (isKeyDown(VK_SHIFT)) t = "www." + t + ".com";
	else if (isKeyDown(VK_CONTROL)) t = "www." + t + ".com";

	while (strleft(t, 1) == " ")
	{
		t = strright(t, strlen(t) -1);
		if (t == " ")
			return;
		
	}
	while (strright(t, 1) == " ")
	{
		t = strleft(t, strlen(t) -1);
		if (t == "")
			return;
		
	}	

	if (!isUrl(t))
		t = "http://search.winamp.com/search/search?invocationType=en00-winamp-553--clientpage&query=" + prepareWebString(t, "+");
	
	urlbox.setText(t);
	myBrowser.navigateUrl(t);
}

Boolean isUrl (String t)
{
	if (t == "about:blank")
		return TRUE;
	
	String ttt = strleft(t, 10);
	// get basically any http:// ftp:// etc
	String slash = System.Strleft("/ ", 1); // the simple slash causes errors on some systems :(
	string backslash = System.Strleft("\\ ", 1);
	if (strsearch(ttt, ":" + slash + slash) > -1) 
		return TRUE;

	 if (strsearch(ttt, ":" + slash) == 1)  // C:/ 	 
		 return TRUE; 	 
	 
	 if (strsearch(ttt, ":" + backslash) == 1){ // C:\ 	 
		 return TRUE; 	 
	 }

	if (strleft(ttt, 2) == backslash+backslash)	// get \\martin-pc
		return TRUE;

	ttt = getToken(t, slash, 0); // get rid of sub dirs
	String v = strright(ttt, 5);
	if (strsearch(v, ".") > -1)
		return TRUE;

	ttt = getToken(t, backslash, 0); // get rid of sub dirs
	String v = strright(ttt, 5);
	if (strsearch(v, ".") > -1)
		return TRUE;
	
	return FALSE;
}

// perpare a webstring out of the searchedit input
// %artist% and %album% are special inputs and will be rendered
// " " will be replaced for browser compatability
string prepareWebString (string url, string replace)
{
	string artist = getPlayItemMetaDataString("artist");
	string album = getPlayItemMetaDataString("album");

	if (artist == "") artist = getPlayitemString();
	if (album == "") album = getPlayitemString();

	url = strReplace(url, "%artist%", artist);
	url = strReplace(url, "%album%", album);

	url = urlEncode(url);

	url = strReplace(url, "%20", replace);

	return url;
}

// a basic function to replace a substring inside a string
string strReplace(string str, string replace, string by)
{
	int pos;
	int len = strlen(replace);
	while (pos = strsearch(str, replace) > -1)
	{
		string str_ = "";
		String _str = "";

		if (pos > 0) str_ = strleft(str, pos);
		
		pos = strlen(str)-pos-len;
		if (pos > 0) _str = strright(str, pos);

		str = str_ + by + _str;
	}
	return str;
}

myGroup.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	myBrowser.onAction (action, param, x, y, p1, p2, source);
}


myBrowser.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	// Open an URL
	if (strlower(action) == "openurl")
	{
		//param = prepareWebString(param, "+");

		urlbox.setText(param);
		myBrowser.navigateUrl(urlbox.getText());

		modeToggle.setActivated(true);
		modeToggle.onToggle(true);

		return;
	}
	// Websearch
	if (strlower(action) == "search")
	{
		urlbox.setText("http://www.google.com/search?client=classicpro&"+param);
		myBrowser.navigateUrl(urlbox.getText());

		modeToggle.setActivated(true);
		modeToggle.onToggle(true);
		
	}
}

myBrowser.onBeforeNavigate(String url, Int flags, String targetframename){
	//String abc = " \n"+integerToString(flags)+" \nTarget= "+targetframename;
	//debug(targetframename);
	//debug("1:::"+ url+ "\n" + integerToString(flags));

	if(!cooldowned){
		bstop.show();
		brefresh.hide();
	}
	
	//debug(targetframename);
	
	if(firstURL!=""){
		if(url==firstURL){
			bback.setXMLParam("ghost", "1");
			bback.setXMLParam("image", "cpro2.reader.back.0");
		}
		else if(bback.getXMLParam("ghost")=="1"){
			bback.setXMLParam("ghost", "0");
			bback.setXMLParam("image", "cpro2.reader.back.1");
		}
	}
}

cooldown.onTimer(){
	cooldown.stop();
	cooldowned=false;
}

myBrowser.onDocumentReady(String url){
	lastestURL=url;
	urlbox.setText(url);

	//debug("2:::"+ url);
	cooldowned = true;
	
	/*
		Ignore all further webpage starts for few ms 
		REASON: Some google+ buttons call a onBeforeNavigate after page is finished loading (www.lyricsoverload.com)
	*/
	cooldown.start();	
	bstop.hide();
	brefresh.show();

	//debug("saved:"+lastURL+"\n\nnuutste:" + lastestURL);
	
	if(!skipForwardHide){
		bffwd.setXMLParam("ghost", "1");
		bffwd.setXMLParam("image", "cpro2.reader.forward.0");
	}
	else if(skipForwardHide && lastURL==lastestURL){
		bffwd.setXMLParam("ghost", "1");
		bffwd.setXMLParam("image", "cpro2.reader.forward.0");
		lastURL="";
	}

	skipForwardHide=false;

	if(firstURL==""){
		if(url!="about:blank") firstURL=url;
	}
	else if(url==firstURL){
		bback.setXMLParam("ghost", "1");
		bback.setXMLParam("image", "cpro2.reader.back.0");
	}
	else if(bback.getXMLParam("ghost")=="1"){
		bback.setXMLParam("ghost", "0");
		bback.setXMLParam("image", "cpro2.reader.back.1");
	}
}
myBrowser.onDocumentComplete(String url){
	//urlbox.setText(url);	
}